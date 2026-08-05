//! Transcribing while the user is still talking.
//!
//! The rule in CLAUDE.md is that ASR runs *during* speech, not after the key
//! comes up. What that buys is not a faster transcription — it is a shorter
//! one: by the time the user lets go, everything up to their last pause has
//! already been recognised, and only the tail is left.
//!
//! The commit boundary is a pause found by VAD, not a fixed window. Whisper is
//! markedly better on a complete phrase than on an arbitrary slice, and a pause
//! is exactly where a phrase ends. That also avoids stitching overlapping
//! windows together token by token, which is where this kind of code usually
//! goes wrong.
//!
//! ```text
//! speech ────pause──── speech ────pause──── speech │ key up
//!        ↑ commit             ↑ commit             ↑ only this is left
//! ```

use crate::asr::{AsrError, TranscribeOptions, Transcriber};
use crate::audio::SAMPLE_RATE;
use crate::vad::{StreamingVad, samples_to_duration};
use std::time::{Duration, Instant};

/// Tuning for the streaming pass.
#[derive(Debug, Clone, Copy)]
pub struct StreamConfig {
    /// How often to re-recognise the uncommitted audio for display. This is
    /// throwaway work whose only purpose is the overlay, so it is deliberately
    /// not frequent.
    pub partial_interval: Duration,

    /// Trailing silence that means "that was a phrase" and triggers a commit.
    pub commit_silence: Duration,

    /// Do not commit until there is at least this much audio.
    ///
    /// This is the setting that decides how much streaming costs in accuracy.
    /// Every commit cuts the audio, and whisper recognises each piece with only
    /// its own context — a fragment of a second or two comes back noticeably
    /// worse than the same words inside a whole phrase.
    ///
    /// What committing early buys is the difference between transcribing the
    /// tail and transcribing everything: about 180 ms on a ten-second
    /// dictation, measured. The budget is 500 ms and the whole pass takes 320.
    /// So there is no reason to fragment anything a single pass handles
    /// comfortably, and the threshold sits above the length of an ordinary
    /// dictation on purpose.
    pub min_commit: Duration,

    /// Commit anyway once the uncommitted audio gets this long. A speaker who
    /// never pauses would otherwise leave the entire dictation to the tail —
    /// and whisper's encoder window is 30 s, past which quality falls away.
    pub max_uncommitted: Duration,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            // Partials are for the overlay and are thrown away. Re-recognising
            // the pending audio competes with the passes that matter, so this
            // is as slow as it can be while still looking live.
            partial_interval: Duration::from_secs(1),
            // Long enough to be the end of a sentence rather than the gap
            // before the next word.
            commit_silence: Duration::from_millis(700),
            min_commit: Duration::from_secs(10),
            max_uncommitted: Duration::from_secs(15),
        }
    }
}

/// What changed after pushing audio in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Update {
    /// Recognised but not final; it can still change. For the overlay.
    Partial(String),
    /// Recognised and settled. This text will not change again.
    Committed(String),
}

/// Where the time went. Logged at the end of every dictation, because the one
/// number that matters is measured here.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StreamStats {
    /// Encoder passes run while the user was still speaking.
    pub commits: u32,
    /// Throwaway passes run for the overlay.
    pub partials: u32,
    /// Audio already recognised by the time the key came up.
    pub committed_audio: Duration,
    /// Audio left to recognise after the key came up. The smaller this is, the
    /// less the user waits.
    pub tail_audio: Duration,
    /// Wall clock spent transcribing after the key came up.
    pub tail_elapsed: Duration,
}

/// One dictation in progress.
pub struct Stream<'a> {
    transcriber: &'a mut dyn Transcriber,
    vad: &'a mut StreamingVad,
    options: TranscribeOptions,
    config: StreamConfig,

    /// Audio recognised into `committed` already removed; this is what is left.
    pending: Vec<f32>,
    committed: Vec<String>,
    partial: String,
    last_partial: Instant,
    stats: StreamStats,
}

impl<'a> Stream<'a> {
    pub fn new(
        transcriber: &'a mut dyn Transcriber,
        vad: &'a mut StreamingVad,
        options: TranscribeOptions,
        config: StreamConfig,
    ) -> Self {
        vad.reset();
        Self {
            transcriber,
            vad,
            options,
            config,
            pending: Vec::new(),
            committed: Vec::new(),
            partial: String::new(),
            last_partial: Instant::now(),
            stats: StreamStats::default(),
        }
    }

    pub const fn stats(&self) -> StreamStats {
        self.stats
    }

    /// The full text as it stands, committed plus the current partial.
    pub fn text(&self) -> String {
        let mut text = self.committed.join(" ");
        if !self.partial.is_empty() {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(&self.partial);
        }
        text
    }

    /// Feed in 16 kHz mono audio. Call it as the capture delivers.
    pub fn push(&mut self, samples: &[f32]) -> Result<Option<Update>, AsrError> {
        self.pending.extend_from_slice(samples);

        let pending_duration = samples_to_duration(self.pending.len());
        if pending_duration < self.config.min_commit {
            return Ok(None);
        }

        // Analyses only the audio it has not seen before, so this stays cheap
        // however long the dictation runs.
        self.vad.advance(&self.pending)?;

        if let Some(split) = self.commit_point(pending_duration)? {
            return self.commit(split).map(Some);
        }

        if self.last_partial.elapsed() >= self.config.partial_interval {
            return self.refresh_partial();
        }

        Ok(None)
    }

    /// The key came up. Recognise what is left and return the whole thing.
    pub fn finish(mut self) -> Result<(String, StreamStats), AsrError> {
        let started = Instant::now();
        self.stats.tail_audio = samples_to_duration(self.pending.len());

        // The one place the one-shot detector is still right: the tail includes
        // the last second, which `advance` has not analysed yet. It runs once
        // per dictation over a couple of seconds, not once per partial over
        // everything.
        let tail = self.vad.inner().trim(&self.pending)?;
        if !tail.is_empty() {
            let options = self.options_with_context();
            let transcript = self.transcriber.transcribe(&tail, &options)?;
            if !transcript.text.is_empty() {
                self.committed.push(transcript.text);
            }
        }

        self.stats.tail_elapsed = started.elapsed();
        self.partial.clear();

        tracing::debug!(
            commits = self.stats.commits,
            partials = self.stats.partials,
            committed_ms = self.stats.committed_audio.as_millis() as u64,
            tail_ms = self.stats.tail_audio.as_millis() as u64,
            tail_elapsed_ms = self.stats.tail_elapsed.as_millis() as u64,
            "stream finished"
        );

        Ok((self.committed.join(" "), self.stats))
    }

    /// What has been recognised so far, as context for the next piece.
    ///
    /// Whisper takes a prompt to bias recognition, and giving it the preceding
    /// words is most of what a fragment loses by being cut out of its sentence.
    /// The window is bounded because the prompt shares the text context with
    /// the output.
    fn context(&self) -> Option<String> {
        const MAX_CHARS: usize = 200;

        let joined = self.committed.join(" ");
        if joined.is_empty() {
            return None;
        }
        let skip = joined.chars().count().saturating_sub(MAX_CHARS);
        Some(joined.chars().skip(skip).collect())
    }

    /// Options for one pass, carrying whatever has been recognised already.
    fn options_with_context(&self) -> TranscribeOptions {
        let mut options = self.options.clone();
        if options.initial_prompt.is_none() {
            options.initial_prompt = self.context();
        }
        options
    }

    /// Where to cut, if anything should be committed yet.
    ///
    /// Returns a sample index into `pending`, always at the end of a speech
    /// segment so a phrase is never split down the middle.
    fn commit_point(&mut self, pending_duration: Duration) -> Result<Option<usize>, AsrError> {
        let segments = self.vad.segments();
        let Some(last) = segments.last().copied() else {
            // Nothing but silence so far. Do not commit it, and do not let it
            // grow without bound either.
            if pending_duration > self.config.max_uncommitted {
                let dropped = self.pending.len();
                self.pending.clear();
                self.vad.drain(dropped);
            }
            return Ok(None);
        };

        let trailing = samples_to_duration(self.pending.len().saturating_sub(last.end));
        let overdue = pending_duration >= self.config.max_uncommitted;

        if trailing >= self.config.commit_silence || overdue {
            if samples_to_duration(last.end) < self.config.min_commit {
                return Ok(None);
            }
            return Ok(Some(last.end));
        }

        Ok(None)
    }

    /// Recognise `pending[..split]`, move its text into the committed list, and
    /// drop that audio.
    fn commit(&mut self, split: usize) -> Result<Update, AsrError> {
        let split = split.min(self.pending.len());
        // Take the speech before dropping the audio and the probabilities that
        // describe it.
        let speech = self.vad.speech(&self.pending, split);
        let head_len = split;
        self.pending.drain(..split);
        self.vad.drain(split);
        self.stats.commits += 1;
        self.stats.committed_audio += samples_to_duration(head_len);

        if !speech.is_empty() {
            let options = self.options_with_context();
            let transcript = self.transcriber.transcribe(&speech, &options)?;
            if !transcript.text.is_empty() {
                self.committed.push(transcript.text);
            }
        }

        // The partial described audio that has now been committed.
        self.partial.clear();
        self.last_partial = Instant::now();

        Ok(Update::Committed(self.text()))
    }

    /// Re-recognise the uncommitted audio for display. Nothing here is final.
    fn refresh_partial(&mut self) -> Result<Option<Update>, AsrError> {
        self.last_partial = Instant::now();

        let speech = self.vad.speech(&self.pending, self.pending.len());
        if speech.is_empty() {
            return Ok(None);
        }

        self.stats.partials += 1;
        let options = self.options_with_context();
        let transcript = self.transcriber.transcribe(&speech, &options)?;
        if transcript.text == self.partial {
            return Ok(None);
        }

        self.partial = transcript.text;
        Ok(Some(Update::Partial(self.text())))
    }
}

/// Samples in one second, for callers sizing their own buffers.
pub const fn samples_per_second() -> usize {
    SAMPLE_RATE as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_commit_gap_is_a_pause_not_a_word_break() {
        let config = StreamConfig::default();
        // 400-500 ms fires between words and cuts sentences in half, which
        // costs visible accuracy — see the note on min_commit.
        assert!(config.commit_silence >= Duration::from_millis(600));
    }

    #[test]
    fn an_ordinary_dictation_is_never_fragmented() {
        // A single pass over ten seconds costs 320 ms against a 500 ms budget,
        // so nothing that short has any reason to be cut up.
        assert!(StreamConfig::default().min_commit >= Duration::from_secs(10));
    }

    #[test]
    fn a_speaker_who_never_pauses_still_gets_committed() {
        let config = StreamConfig::default();
        assert!(
            config.max_uncommitted < Duration::from_secs(30),
            "whisper's window is 30 s"
        );
        assert!(config.max_uncommitted > config.min_commit);
    }

    #[test]
    fn samples_per_second_matches_the_pipeline_rate() {
        assert_eq!(samples_per_second(), SAMPLE_RATE as usize);
    }

    #[test]
    fn stats_start_empty() {
        let stats = StreamStats::default();
        assert_eq!(stats.commits, 0);
        assert_eq!(stats.tail_audio, Duration::ZERO);
    }
}
