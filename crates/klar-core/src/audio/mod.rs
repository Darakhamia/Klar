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
