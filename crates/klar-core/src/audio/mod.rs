//! Getting audio off the microphone and into the shape whisper wants.

pub mod capture;
pub mod resample;
pub mod wav;

pub use capture::{
    Capture, CaptureConfig, Device, SourceFormat, capture_devices, to_whisper_input,
};
pub use resample::{downmix_to_mono, resample_mono};

/// What whisper.cpp expects, and the only rate anything downstream of capture
/// ever sees: 16 kHz, mono, f32 in -1.0..=1.0.
pub const SAMPLE_RATE: u32 = 16_000;

#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("no input device available")]
    NoDevice,

    #[error("input device '{name}' not found")]
    UnknownDevice { name: String },

    #[error("the input device offers no usable configuration: {0}")]
    NoConfig(String),

    #[error("could not open the input stream: {0}")]
    Stream(String),

    #[error("resampling {from} Hz to {to} Hz failed: {reason}")]
    Resample { from: u32, to: u32, reason: String },

    #[error("wav: {0}")]
    Wav(String),

    #[error("capture stopped unexpectedly")]
    Disconnected,
}

/// Samples per second → samples. Used everywhere a duration has to become a
/// buffer length.
pub const fn samples_for(seconds_x1000: u64) -> usize {
    (SAMPLE_RATE as u64 * seconds_x1000 / 1000) as usize
}

/// Turns a trickle of capture callbacks into 16 kHz mono, a block at a time.
///
/// The resampler is built per call, so converting each 20 ms callback on its
/// own would leave a discontinuity at every boundary. Converting the whole
/// buffer again on every callback would be quadratic. This does neither: it
/// holds the leftover raw samples and converts once enough have arrived,
/// always on a whole number of frames so the channels stay aligned.
pub struct BlockConverter {
    format: capture::SourceFormat,
    /// Raw interleaved samples not yet converted.
    leftover: Vec<f32>,
    block_frames: usize,
}

impl BlockConverter {
    /// `block` is how much audio to gather before converting. Larger means
    /// fewer, cheaper conversions and a coarser boundary artefact; 250 ms is
    /// well inside what the streaming pass needs and inaudible to whisper.
    pub fn new(format: capture::SourceFormat, block: std::time::Duration) -> Self {
        let block_frames = (block.as_secs_f64() * f64::from(format.sample_rate)).round() as usize;
        Self {
            format,
            leftover: Vec::new(),
            block_frames: block_frames.max(1),
        }
    }

    /// Add raw interleaved samples.
    ///
    /// Returns nothing until at least one block has accumulated, then converts
    /// everything whole frames allow — holding back a spare block would just
    /// delay the audio for no benefit.
    pub fn push(&mut self, raw: &[f32]) -> Result<Vec<f32>, AudioError> {
        self.leftover.extend_from_slice(raw);

        let channels = usize::from(self.format.channels.max(1));
        let ready_frames = self.leftover.len() / channels;
        if ready_frames < self.block_frames {
            return Ok(Vec::new());
        }

        let take = ready_frames * channels;
        let block: Vec<f32> = self.leftover.drain(..take).collect();
        capture::to_whisper_input(&block, self.format)
    }

    /// Convert whatever is left, however short. For the end of a dictation.
    pub fn flush(&mut self) -> Result<Vec<f32>, AudioError> {
        if self.leftover.is_empty() {
            return Ok(Vec::new());
        }
        let channels = usize::from(self.format.channels.max(1));
        // Drop a ragged partial frame rather than shifting every later one.
        let take = (self.leftover.len() / channels) * channels;
        let block: Vec<f32> = self.leftover.drain(..take).collect();
        self.leftover.clear();
        capture::to_whisper_input(&block, self.format)
    }
}

/// Peak level of a block, 0.0..=1.0. The overlay's waveform is driven from this.
pub fn peak(samples: &[f32]) -> f32 {
    samples
        .iter()
        .fold(0.0_f32, |acc, s| acc.max(s.abs()))
        .min(1.0)
}

/// Root-mean-square level of a block. Steadier than the peak, so it is what the
/// "is anything reaching the microphone at all" check in onboarding uses.
pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f64 = samples.iter().map(|s| f64::from(*s) * f64::from(*s)).sum();
    (sum / samples.len() as f64).sqrt() as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_converter_holds_back_until_a_block_is_ready() {
        let format = capture::SourceFormat {
            sample_rate: 16_000,
            channels: 1,
        };
        let mut converter = BlockConverter::new(format, std::time::Duration::from_millis(250));

        // 100 ms is not a block.
        assert!(converter.push(&vec![0.1; 1_600]).unwrap().is_empty());
        // 200 ms still is not.
        assert!(converter.push(&vec![0.1; 1_600]).unwrap().is_empty());
        // 300 ms crosses the threshold, and everything accumulated comes out.
        let out = converter.push(&vec![0.1; 1_600]).unwrap();
        assert_eq!(out.len(), 4_800, "all three pushes, not just one block");
    }

    #[test]
    fn the_converter_flushes_the_remainder() {
        let format = capture::SourceFormat {
            sample_rate: 16_000,
            channels: 1,
        };
        let mut converter = BlockConverter::new(format, std::time::Duration::from_millis(250));
        converter.push(&vec![0.1; 1_000]).unwrap();
        assert_eq!(converter.flush().unwrap().len(), 1_000);
        assert!(
            converter.flush().unwrap().is_empty(),
            "flushing twice yields nothing"
        );
    }

    #[test]
    fn the_converter_keeps_channels_aligned() {
        let format = capture::SourceFormat {
            sample_rate: 16_000,
            channels: 2,
        };
        let mut converter = BlockConverter::new(format, std::time::Duration::from_millis(100));
        // 3201 samples is 1600 whole stereo frames plus a stray one.
        let out = converter.push(&vec![0.5; 3_201]).unwrap();
        assert_eq!(
            out.len(),
            1_600,
            "the odd sample must not shift the channels"
        );
    }

    #[test]
    fn everything_pushed_comes_back_out() {
        let format = capture::SourceFormat {
            sample_rate: 16_000,
            channels: 1,
        };
        let mut converter = BlockConverter::new(format, std::time::Duration::from_millis(250));
        let mut total = 0;
        for _ in 0..10 {
            total += converter.push(&vec![0.2; 1_000]).unwrap().len();
        }
        total += converter.flush().unwrap().len();
        assert_eq!(total, 10_000, "no samples may be lost between blocks");
    }

    #[test]
    fn samples_for_matches_the_rate() {
        assert_eq!(samples_for(1_000), 16_000);
        assert_eq!(samples_for(500), 8_000);
        assert_eq!(samples_for(0), 0);
    }

    #[test]
    fn peak_is_absolute_and_clamped() {
        assert_eq!(peak(&[]), 0.0);
        assert_eq!(peak(&[0.2, -0.7, 0.5]), 0.7);
        assert_eq!(peak(&[3.0]), 1.0);
    }

    #[test]
    fn rms_of_full_scale_square_is_one() {
        let square: Vec<f32> = (0..1000)
            .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
            .collect();
        assert!((rms(&square) - 1.0).abs() < 1e-6);
        assert_eq!(rms(&[]), 0.0);
    }
}
