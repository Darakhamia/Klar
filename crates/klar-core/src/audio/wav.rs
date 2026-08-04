//! Reading and writing 16 kHz mono WAV.
//!
//! Two uses, both explicit:
//!
//! - **Fixtures.** `klar-cli transcribe file.wav` runs the ASR stage against a
//!   known clip, so transcription quality and timing can be measured without a
//!   microphone in the loop.
//! - **Debug capture.** Writing recorded audio to disk is the one exception to
//!   the privacy rule, and it only happens when the caller reaches for
//!   [`write_mono`] directly. Nothing in the pipeline calls it.

use super::{AudioError, SAMPLE_RATE, downmix_to_mono, resample_mono};
use std::path::Path;

/// Read any WAV and hand back 16 kHz mono, resampling and downmixing as needed.
pub fn read_as_whisper_input(path: &Path) -> Result<Vec<f32>, AudioError> {
    let mut reader = hound::WavReader::open(path)
        .map_err(|e| AudioError::Wav(format!("{}: {e}", path.display())))?;
    let spec = reader.spec();

    let interleaved: Vec<f32> = match (spec.sample_format, spec.bits_per_sample) {
        (hound::SampleFormat::Float, 32) => reader
            .samples::<f32>()
            .collect::<Result<_, _>>()
            .map_err(|e| AudioError::Wav(e.to_string()))?,
        (hound::SampleFormat::Int, bits) => {
            // Full scale for the declared depth, so 24-bit does not come out
            // 256× too quiet.
            let scale = f32::from(i16::MAX) * 2.0_f32.powi(i32::from(bits) - 16);
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 / scale))
                .collect::<Result<_, _>>()
                .map_err(|e| AudioError::Wav(e.to_string()))?
        }
        (format, bits) => {
            return Err(AudioError::Wav(format!(
                "unsupported wav format {format:?}/{bits}-bit"
            )));
        }
    };

    let mono = downmix_to_mono(&interleaved, spec.channels);
    resample_mono(&mono, spec.sample_rate)
}

/// Write 16 kHz mono samples as 16-bit PCM.
///
/// Only called from an explicit debug path. Audio does not otherwise touch the
/// disk.
pub fn write_mono(path: &Path, samples: &[f32]) -> Result<(), AudioError> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::create(path, spec)
        .map_err(|e| AudioError::Wav(format!("{}: {e}", path.display())))?;

    for sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        // 32767 rather than 32768: scaling by the latter turns a legitimate
        // -1.0 into a value that wraps on the way to i16.
        let encoded = (clamped * f32::from(i16::MAX)).round() as i16;
        writer
            .write_sample(encoded)
            .map_err(|e| AudioError::Wav(e.to_string()))?;
    }

    writer
        .finalize()
        .map_err(|e| AudioError::Wav(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("klar-wav-tests");
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn a_written_file_reads_back_as_the_same_audio() {
        let path = temp_path("roundtrip.wav");
        let original: Vec<f32> = (0..16_000)
            .map(|i| ((i as f32) * 0.01).sin() * 0.8)
            .collect();

        write_mono(&path, &original).unwrap();
        let read_back = read_as_whisper_input(&path).unwrap();

        assert_eq!(read_back.len(), original.len());
        for (a, b) in original.iter().zip(&read_back) {
            // 16-bit quantisation is the only loss.
            assert!((a - b).abs() < 1.0 / 32_767.0 + 1e-6, "{a} vs {b}");
        }
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn full_scale_survives_the_round_trip_without_wrapping() {
        let path = temp_path("fullscale.wav");
        write_mono(&path, &[1.0, -1.0, 0.0]).unwrap();
        let read_back = read_as_whisper_input(&path).unwrap();
        assert!(
            read_back[0] > 0.99,
            "positive peak wrapped: {}",
            read_back[0]
        );
        assert!(
            read_back[1] < -0.99,
            "negative peak wrapped: {}",
            read_back[1]
        );
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn out_of_range_input_is_clamped_not_wrapped() {
        let path = temp_path("clamp.wav");
        write_mono(&path, &[4.0, -4.0]).unwrap();
        let read_back = read_as_whisper_input(&path).unwrap();
        assert!(read_back[0] > 0.99 && read_back[1] < -0.99, "{read_back:?}");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn a_missing_file_is_an_error_not_a_panic() {
        assert!(read_as_whisper_input(Path::new("/nonexistent/klar.wav")).is_err());
    }
}
