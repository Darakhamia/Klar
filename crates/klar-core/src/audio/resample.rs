//! Whatever the device gives us → 16 kHz mono.
//!
//! Microphones hand back 44.1 or 48 kHz, often stereo, sometimes i16. Whisper
//! wants 16 kHz mono f32 and nothing else, so every path through capture ends
//! here.
//!
//! The interpolator is polynomial rather than sinc. At this ratio and for
//! speech the audible difference is nil, and the cost is not: this runs on
//! every window during M3's streaming pass, inside a budget where 400 ms buys
//! the whole transcription.

use super::{AudioError, SAMPLE_RATE};
use rubato::audioadapter_buffers::direct::SequentialSliceOfVecs;
use rubato::{Async, FixedAsync, PolynomialDegree, Resampler};

/// Interleaved multi-channel → mono, by averaging. Averaging rather than taking
/// the first channel: a headset that only carries the voice on the right would
/// otherwise come out silent.
pub fn downmix_to_mono(interleaved: &[f32], channels: u16) -> Vec<f32> {
    let channels = usize::from(channels.max(1));
    if channels == 1 {
        return interleaved.to_vec();
    }
    interleaved
        .chunks_exact(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

/// Mono at `from_rate` → mono at 16 kHz.
///
/// Returns the input untouched when it is already at the target rate, which is
/// the common case on Windows once the device is opened at 16 kHz directly.
pub fn resample_mono(input: &[f32], from_rate: u32) -> Result<Vec<f32>, AudioError> {
    resample_mono_to(input, from_rate, SAMPLE_RATE)
}

/// The general form. Separate so the tests can go both up and down.
pub fn resample_mono_to(
    input: &[f32],
    from_rate: u32,
    to_rate: u32,
) -> Result<Vec<f32>, AudioError> {
    let fail = |reason: String| AudioError::Resample {
        from: from_rate,
        to: to_rate,
        reason,
    };

    if from_rate == 0 || to_rate == 0 {
        return Err(fail("a sample rate of zero is not a rate".into()));
    }
    if from_rate == to_rate || input.is_empty() {
        return Ok(input.to_vec());
    }

    let ratio = f64::from(to_rate) / f64::from(from_rate);
    let mut resampler = Async::<f32>::new_poly(
        ratio,
        // The ratio never moves after construction; M3's streaming pass keeps a
        // resampler per session rather than retuning one.
        1.0,
        PolynomialDegree::Cubic,
        1024,
        1,
        FixedAsync::Input,
    )
    .map_err(|e| fail(e.to_string()))?;

    let input_channels = vec![input.to_vec()];
    let source = SequentialSliceOfVecs::new(&input_channels, 1, input.len())
        .map_err(|e| fail(e.to_string()))?;

    let capacity = resampler.process_all_needed_output_len(input.len());
    let mut output_channels = vec![vec![0.0_f32; capacity]];
    let mut sink = SequentialSliceOfVecs::new_mut(&mut output_channels, 1, capacity)
        .map_err(|e| fail(e.to_string()))?;

    let (_consumed, produced) = resampler
        .process_all_into_buffer(&source, &mut sink, input.len(), None)
        .map_err(|e| fail(e.to_string()))?;

    let mut out = output_channels.swap_remove(0);
    out.truncate(produced);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    fn sine(freq: f32, rate: u32, seconds: f32) -> Vec<f32> {
        let n = (rate as f32 * seconds) as usize;
        (0..n)
            .map(|i| (TAU * freq * i as f32 / rate as f32).sin() * 0.5)
            .collect()
    }

    /// Estimate a pure tone's frequency by counting rising zero crossings.
    fn dominant_freq(samples: &[f32], rate: u32) -> f32 {
        let crossings = samples
            .windows(2)
            .filter(|w| w[0] <= 0.0 && w[1] > 0.0)
            .count();
        crossings as f32 * rate as f32 / samples.len() as f32
    }

    #[test]
    fn mono_input_passes_through_the_downmix() {
        let input = vec![0.1, -0.2, 0.3];
        assert_eq!(downmix_to_mono(&input, 1), input);
    }

    #[test]
    fn stereo_is_averaged_not_dropped() {
        // Left silent, right carrying the signal — a real headset layout.
        let interleaved = vec![0.0, 1.0, 0.0, -1.0];
        assert_eq!(downmix_to_mono(&interleaved, 2), vec![0.5, -0.5]);
    }

    #[test]
    fn ragged_tail_is_discarded_rather_than_misaligned() {
        // Two full frames plus a stray sample: dropping it beats shifting every
        // later frame by one channel.
        let interleaved = vec![1.0, 1.0, 2.0, 2.0, 3.0];
        assert_eq!(downmix_to_mono(&interleaved, 2), vec![1.0, 2.0]);
    }

    #[test]
    fn same_rate_is_a_passthrough() {
        let input = sine(440.0, 16_000, 0.1);
        let out = resample_mono(&input, 16_000).unwrap();
        assert_eq!(out, input);
    }

    #[test]
    fn empty_input_stays_empty() {
        assert!(resample_mono(&[], 48_000).unwrap().is_empty());
    }

    #[test]
    fn zero_rate_is_rejected_not_divided_by() {
        assert!(resample_mono_to(&[0.0; 16], 0, 16_000).is_err());
        assert!(resample_mono_to(&[0.0; 16], 16_000, 0).is_err());
    }

    #[test]
    fn downsampling_48k_to_16k_gives_a_third_of_the_samples() {
        let input = sine(440.0, 48_000, 1.0);
        let out = resample_mono(&input, 48_000).unwrap();
        let expected = 16_000_f32;
        // Polynomial interpolation drops a few frames of warm-up; a percent of
        // slack covers it without hiding a real ratio error.
        assert!(
            (out.len() as f32 - expected).abs() < expected * 0.01,
            "expected ~{expected} samples, got {}",
            out.len()
        );
    }

    #[test]
    fn a_tone_keeps_its_pitch_across_the_rate_change() {
        let input = sine(440.0, 44_100, 1.0);
        let out = resample_mono(&input, 44_100).unwrap();
        let freq = dominant_freq(&out, SAMPLE_RATE);
        assert!(
            (freq - 440.0).abs() < 5.0,
            "expected ~440 Hz, measured {freq}"
        );
    }

    #[test]
    fn upsampling_works_too() {
        let input = sine(300.0, 8_000, 1.0);
        let out = resample_mono_to(&input, 8_000, 16_000).unwrap();
        assert!((out.len() as f32 - 16_000.0).abs() < 160.0);
        let freq = dominant_freq(&out, 16_000);
        assert!(
            (freq - 300.0).abs() < 5.0,
            "expected ~300 Hz, measured {freq}"
        );
    }
}
