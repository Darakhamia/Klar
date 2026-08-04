//! Voice activity detection.
//!
//! Two jobs in the pipeline:
//!
//! - **Finding the end of an utterance.** A pause long enough to be a sentence
//!   boundary is where the streaming pass commits what it has, so that key
//!   release only leaves a short tail to transcribe.
//! - **Trimming silence.** Whisper given a buffer that is mostly silence
//!   invents text to fill it, and pays for the empty seconds either way.
//!
//! The detector is Silero, run through whisper.cpp's own VAD rather than a
//! separate ONNX runtime — see `docs/platform-notes.md` for why.

use std::path::Path;
use std::time::Duration;
use whisper_rs::{WhisperVadContext, WhisperVadContextParams, WhisperVadParams};

use crate::asr::{AsrError, Backend};
use crate::audio::SAMPLE_RATE;

/// A stretch of speech, in samples relative to the buffer it was found in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Segment {
    pub start: usize,
    pub end: usize,
}

impl Segment {
    pub const fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn duration(&self) -> Duration {
        samples_to_duration(self.len())
    }
}

/// How eagerly to call something speech.
#[derive(Debug, Clone, Copy)]
pub struct VadSettings {
    /// 0..1. Silero's own probability threshold.
    pub threshold: f32,
    /// Bursts of "speech" shorter than this are noise — a chair, a keyboard.
    pub min_speech: Duration,
    /// Silence shorter than this is a gap within a sentence, not the end of one.
    pub min_silence: Duration,
    /// Keep a little audio either side of each segment, so a cut does not clip
    /// the start of a word.
    pub pad: Duration,
}

impl Default for VadSettings {
    fn default() -> Self {
        Self {
            threshold: 0.5,
            min_speech: Duration::from_millis(250),
            // Longer than Silero's 100 ms default: at 100 ms every gap between
            // words looks like the end of a sentence, and the stream would
            // commit mid-phrase.
            min_silence: Duration::from_millis(400),
            pad: Duration::from_millis(60),
        }
    }
}

pub struct Vad {
    /// Reached directly by [`StreamingVad`], which drives the detector a block
    /// at a time rather than through the one-shot helpers below.
    context: WhisperVadContext,
    settings: VadSettings,
}

impl Vad {
    /// Load the Silero model. Uses the GPU when the build has one, since it
    /// runs on the same ggml backend whisper does.
    pub fn load(model: &Path, settings: VadSettings) -> Result<Self, AsrError> {
        if !model.is_file() {
            return Err(AsrError::ModelMissing(model.to_path_buf()));
        }

        let path = model.to_str().ok_or_else(|| {
            AsrError::Load(format!(
                "VAD model path is not valid UTF-8: {}",
                model.display()
            ))
        })?;

        let mut params = WhisperVadContextParams::new();
        params.set_use_gpu(Backend::compiled().is_gpu());

        let context = WhisperVadContext::new(path, params)
            .map_err(|e| AsrError::Load(format!("VAD: {e}")))?;

        tracing::info!(model = %model.display(), "vad ready");
        Ok(Self { context, settings })
    }

    pub const fn settings(&self) -> &VadSettings {
        &self.settings
    }

    /// Find the speech in `samples`, which must be 16 kHz mono.
    pub fn segments(&mut self, samples: &[f32]) -> Result<Vec<Segment>, AsrError> {
        // Silero needs a window to work with; below that everything looks like
        // silence and saying so is more honest than guessing.
        if samples.len() < SAMPLE_RATE as usize / 10 {
            return Ok(Vec::new());
        }

        let mut params = WhisperVadParams::new();
        params.set_threshold(self.settings.threshold);
        params.set_min_speech_duration(millis(self.settings.min_speech));
        params.set_min_silence_duration(millis(self.settings.min_silence));
        params.set_speech_pad(millis(self.settings.pad));

        let found = self
            .context
            .segments_from_samples(params, samples)
            .map_err(|e| AsrError::Inference(format!("VAD: {e}")))?;

        let mut segments = Vec::new();
        for index in 0..found.num_segments() {
            let (Some(start), Some(end)) = (
                found.get_segment_start_timestamp(index),
                found.get_segment_end_timestamp(index),
            ) else {
                continue;
            };
            // whisper.cpp reports centiseconds.
            let segment = Segment {
                start: centiseconds_to_samples(start).min(samples.len()),
                end: centiseconds_to_samples(end).min(samples.len()),
            };
            if !segment.is_empty() {
                segments.push(segment);
            }
        }

        Ok(segments)
    }

    /// How much silence there is at the end of `samples`.
    ///
    /// The stream commits when this gets long enough to be a sentence boundary.
    /// `None` when there is no speech in the buffer at all — that is not the
    /// same as a long pause, and treating it as one would commit silence.
    pub fn trailing_silence(&mut self, samples: &[f32]) -> Result<Option<Duration>, AsrError> {
        let segments = self.segments(samples)?;
        let Some(last) = segments.last() else {
            return Ok(None);
        };
        Ok(Some(samples_to_duration(
            samples.len().saturating_sub(last.end),
        )))
    }

    /// Drop everything that is not speech, keeping the segments in order.
    ///
    /// Used before a final transcription: whisper charges for silence and
    /// hallucinates into it.
    pub fn trim(&mut self, samples: &[f32]) -> Result<Vec<f32>, AsrError> {
        let segments = self.segments(samples)?;
        if segments.is_empty() {
            return Ok(Vec::new());
        }
        let mut kept = Vec::with_capacity(segments.iter().map(Segment::len).sum());
        for segment in segments {
            kept.extend_from_slice(&samples[segment.start..segment.end]);
        }
        Ok(kept)
    }
}

/// Silero looks at the audio 512 samples at a time, so every probability it
/// reports covers 32 ms.
pub const FRAME: usize = 512;

/// Analyse in blocks of roughly this much new audio.
const ANALYSE_BLOCK: usize = SAMPLE_RATE as usize;

/// Replay this much already-analysed audio before each block. Silero is
/// recurrent: starting cold at a block boundary costs accuracy for the first
/// frames, and this is the cheapest way to avoid it.
const WARMUP: usize = SAMPLE_RATE as usize;

/// Voice activity over a buffer that keeps growing.
///
/// Running the whole detector over the whole buffer on every push is what the
/// obvious implementation does, and it costs ~400 ms per call by the time the
/// buffer is ten seconds long — far past the point of being usable on the audio
/// path. This analyses each second of audio exactly once and keeps the
/// per-frame probabilities, so segmentation afterwards is arithmetic.
pub struct StreamingVad {
    vad: Vad,
    /// One probability per 32 ms frame, for all analysed audio.
    probs: Vec<f32>,
    /// How much of the buffer the probabilities cover.
    analysed: usize,
}

impl StreamingVad {
    pub const fn new(vad: Vad) -> Self {
        Self {
            vad,
            probs: Vec::new(),
            analysed: 0,
        }
    }

    pub const fn settings(&self) -> &VadSettings {
        self.vad.settings()
    }

    /// Bring the analysis up to date with `buffer`, which must be the same
    /// buffer, only longer, on every call.
    pub fn advance(&mut self, buffer: &[f32]) -> Result<(), AsrError> {
        while buffer.len() - self.analysed >= ANALYSE_BLOCK {
            let block_end = self.analysed + ANALYSE_BLOCK;
            let window_start = self.analysed.saturating_sub(WARMUP);

            let window = &buffer[window_start..block_end];
            self.vad
                .context
                .detect_speech(window)
                .map_err(|e| AsrError::Inference(format!("VAD: {e}")))?;

            // Keep only the frames covering the new audio; the rest was warm-up.
            let all = self.vad.context.probabilities();
            let new_frames = ANALYSE_BLOCK / FRAME;
            let start = all.len().saturating_sub(new_frames);
            self.probs.extend_from_slice(&all[start..]);

            self.analysed = block_end;
        }
        Ok(())
    }

    /// Forget the first `samples` of the buffer, because they have been
    /// committed and dropped.
    pub fn drain(&mut self, samples: usize) {
        let frames = samples / FRAME;
        self.probs.drain(..frames.min(self.probs.len()));
        self.analysed = self.analysed.saturating_sub(frames * FRAME);
    }

    pub fn reset(&mut self) {
        self.probs.clear();
        self.analysed = 0;
    }

    /// Speech segments over the analysed audio.
    pub fn segments(&self) -> Vec<Segment> {
        segments_from_probs(&self.probs, self.vad.settings())
    }

    /// Silence at the end of the analysed audio, or `None` when no speech has
    /// been found yet — which is not the same thing.
    pub fn trailing_silence(&self) -> Option<Duration> {
        let last = self.segments().last().copied()?;
        Some(samples_to_duration(self.analysed.saturating_sub(last.end)))
    }

    /// The underlying detector, for the one-shot calls that finalisation makes.
    pub fn inner(&mut self) -> &mut Vad {
        &mut self.vad
    }
}

/// Turn per-frame probabilities into segments.
///
/// Deliberately plain arithmetic with no model involved, so the rules that
/// decide where a phrase ends can be tested directly.
pub fn segments_from_probs(probs: &[f32], settings: &VadSettings) -> Vec<Segment> {
    let min_speech_frames = frames(settings.min_speech).max(1);
    let min_silence_frames = frames(settings.min_silence).max(1);
    let pad_frames = frames(settings.pad);

    let mut segments: Vec<Segment> = Vec::new();
    let mut speech_start: Option<usize> = None;
    let mut silence_run = 0_usize;

    for (index, probability) in probs.iter().enumerate() {
        if *probability >= settings.threshold {
            silence_run = 0;
            if speech_start.is_none() {
                speech_start = Some(index);
            }
        } else if let Some(start) = speech_start {
            silence_run += 1;
            if silence_run >= min_silence_frames {
                let end = index + 1 - silence_run;
                push_segment(
                    &mut segments,
                    start,
                    end,
                    min_speech_frames,
                    pad_frames,
                    probs.len(),
                );
                speech_start = None;
                silence_run = 0;
            }
        }
    }

    if let Some(start) = speech_start {
        push_segment(
            &mut segments,
            start,
            probs.len(),
            min_speech_frames,
            pad_frames,
            probs.len(),
        );
    }

    segments
}

fn push_segment(
    segments: &mut Vec<Segment>,
    start_frame: usize,
    end_frame: usize,
    min_speech_frames: usize,
    pad_frames: usize,
    total_frames: usize,
) {
    if end_frame.saturating_sub(start_frame) < min_speech_frames {
        return;
    }
    let start = start_frame.saturating_sub(pad_frames) * FRAME;
    let end = (end_frame + pad_frames).min(total_frames) * FRAME;
    segments.push(Segment { start, end });
}

fn frames(duration: Duration) -> usize {
    (duration.as_secs_f64() * f64::from(SAMPLE_RATE) / FRAME as f64).round() as usize
}

const fn millis(duration: Duration) -> std::ffi::c_int {
    duration.as_millis() as std::ffi::c_int
}

fn centiseconds_to_samples(centiseconds: f32) -> usize {
    ((f64::from(centiseconds) / 100.0) * f64::from(SAMPLE_RATE))
        .round()
        .max(0.0) as usize
}

pub fn samples_to_duration(samples: usize) -> Duration {
    Duration::from_secs_f64(samples as f64 / f64::from(SAMPLE_RATE))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_segment_knows_its_length_and_duration() {
        let segment = Segment {
            start: 0,
            end: SAMPLE_RATE as usize,
        };
        assert_eq!(segment.len(), 16_000);
        assert_eq!(segment.duration(), Duration::from_secs(1));
        assert!(!segment.is_empty());
    }

    #[test]
    fn an_inverted_segment_is_empty_rather_than_negative() {
        let segment = Segment {
            start: 100,
            end: 50,
        };
        assert_eq!(segment.len(), 0);
        assert!(segment.is_empty());
    }

    #[test]
    fn centiseconds_convert_at_the_sample_rate() {
        assert_eq!(centiseconds_to_samples(0.0), 0);
        assert_eq!(centiseconds_to_samples(100.0), 16_000);
        assert_eq!(centiseconds_to_samples(50.0), 8_000);
    }

    #[test]
    fn samples_convert_back_to_time() {
        assert_eq!(samples_to_duration(16_000), Duration::from_secs(1));
        assert_eq!(samples_to_duration(0), Duration::ZERO);
    }

    #[test]
    fn the_default_silence_gap_is_longer_than_a_pause_between_words() {
        // 100 ms — Silero's own default — fires between words and would make
        // the stream commit mid-sentence.
        assert!(VadSettings::default().min_silence >= Duration::from_millis(300));
    }

    /// Build a probability track from a description in frames.
    fn probs(spec: &[(f32, usize)]) -> Vec<f32> {
        spec.iter()
            .flat_map(|(p, n)| std::iter::repeat_n(*p, *n))
            .collect()
    }

    fn settings(min_speech_ms: u64, min_silence_ms: u64, pad_ms: u64) -> VadSettings {
        VadSettings {
            threshold: 0.5,
            min_speech: Duration::from_millis(min_speech_ms),
            min_silence: Duration::from_millis(min_silence_ms),
            pad: Duration::from_millis(pad_ms),
        }
    }

    #[test]
    fn silence_alone_produces_no_segments() {
        let track = probs(&[(0.0, 100)]);
        assert!(segments_from_probs(&track, &settings(250, 400, 0)).is_empty());
    }

    #[test]
    fn one_stretch_of_speech_is_one_segment() {
        // 32 frames ≈ 1.0 s of speech between silences.
        let track = probs(&[(0.0, 20), (0.9, 32), (0.0, 40)]);
        let found = segments_from_probs(&track, &settings(250, 400, 0));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].start, 20 * FRAME);
        assert_eq!(found[0].end, 52 * FRAME);
    }

    #[test]
    fn a_short_gap_does_not_split_a_phrase() {
        // 6 frames of silence ≈ 190 ms, under the 400 ms threshold.
        let track = probs(&[(0.9, 20), (0.0, 6), (0.9, 20), (0.0, 40)]);
        let found = segments_from_probs(&track, &settings(250, 400, 0));
        assert_eq!(
            found.len(),
            1,
            "a gap between words must not end the phrase"
        );
    }

    #[test]
    fn a_real_pause_splits_two_phrases() {
        // 20 frames of silence ≈ 640 ms, over the threshold.
        let track = probs(&[(0.9, 20), (0.0, 20), (0.9, 20), (0.0, 40)]);
        let found = segments_from_probs(&track, &settings(250, 400, 0));
        assert_eq!(found.len(), 2);
        assert!(found[0].end < found[1].start);
    }

    #[test]
    fn a_click_too_short_to_be_speech_is_dropped() {
        // 3 frames ≈ 96 ms, under the 250 ms minimum.
        let track = probs(&[(0.0, 20), (0.9, 3), (0.0, 40)]);
        assert!(segments_from_probs(&track, &settings(250, 400, 0)).is_empty());
    }

    #[test]
    fn padding_widens_a_segment_without_running_off_the_end() {
        let track = probs(&[(0.9, 32)]);
        let found = segments_from_probs(&track, &settings(250, 400, 200));
        assert_eq!(found.len(), 1);
        // Nothing before frame 0 and nothing past the end of the track.
        assert_eq!(found[0].start, 0);
        assert_eq!(found[0].end, 32 * FRAME);
    }

    #[test]
    fn speech_still_going_at_the_end_is_reported() {
        let track = probs(&[(0.0, 10), (0.9, 30)]);
        let found = segments_from_probs(&track, &settings(250, 400, 0));
        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0].end,
            40 * FRAME,
            "an unfinished phrase runs to the end"
        );
    }

    #[test]
    fn the_threshold_is_respected() {
        let quiet = probs(&[(0.4, 40)]);
        assert!(segments_from_probs(&quiet, &settings(250, 400, 0)).is_empty());

        let loud = probs(&[(0.6, 40)]);
        assert_eq!(segments_from_probs(&loud, &settings(250, 400, 0)).len(), 1);
    }

    #[test]
    fn frames_convert_at_the_frame_size() {
        // 512 samples at 16 kHz is 32 ms, so a second is 31.25 → 31 frames.
        assert_eq!(frames(Duration::from_millis(32)), 1);
        assert_eq!(frames(Duration::from_secs(1)), 31);
        assert_eq!(frames(Duration::ZERO), 0);
    }

    #[test]
    fn a_missing_model_is_reported_rather_than_loaded() {
        let error = Vad::load(Path::new("/nonexistent/vad.bin"), VadSettings::default())
            .err()
            .unwrap_or_else(|| panic!("a nonexistent path must not load"));
        assert!(matches!(error, AsrError::ModelMissing(_)), "got {error:?}");
    }
}
