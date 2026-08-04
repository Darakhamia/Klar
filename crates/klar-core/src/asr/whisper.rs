//! whisper.cpp, behind [`Transcriber`].
//!
//! The one thing this module has to get right beyond producing text is being
//! honest about the backend. A build that quietly falls back to CPU still
//! transcribes — it just misses the latency budget by an order of magnitude,
//! and the cause will not surface until someone measures at M3. So the load
//! path logs what was compiled in, what whisper.cpp reports about itself, and
//! whether the GPU was requested, and it routes whisper.cpp's own device
//! initialisation lines into our log rather than swallowing them.

use super::{AsrError, Backend, TranscribeOptions, Transcriber, Transcript};
use crate::audio::SAMPLE_RATE;
use std::path::Path;
use std::sync::Once;
use std::time::{Duration, Instant};
use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState,
};

static LOGGING_HOOKS: Once = Once::new();

pub struct WhisperTranscriber {
    state: WhisperState,
    backend: Backend,
    default_threads: usize,
    /// English-only models (`*.en`) still run whisper's language detector if
    /// asked to, and it returns noise — a confident-looking `sq` at p = 0.01.
    /// Knowing the model's kind lets us skip the pass entirely.
    multilingual: bool,
}

impl WhisperTranscriber {
    /// Load a model and report, loudly, what we are running on.
    pub fn load(model: &Path) -> Result<Self, AsrError> {
        // Send whisper.cpp's and ggml's own logs — including the device
        // registration lines that are the only runtime proof a GPU was picked
        // up — into tracing, so they land in Klar's log file.
        LOGGING_HOOKS.call_once(whisper_rs::install_logging_hooks);

        if !model.is_file() {
            return Err(AsrError::ModelMissing(model.to_path_buf()));
        }

        let backend = Backend::compiled();
        let mut params = WhisperContextParameters::default();
        params.use_gpu(backend.is_gpu());

        let model_path = model.to_str().ok_or_else(|| {
            AsrError::Load(format!(
                "model path is not valid UTF-8: {}",
                model.display()
            ))
        })?;

        let started = Instant::now();
        let context = WhisperContext::new_with_params(model_path, params)
            .map_err(|e| AsrError::Load(e.to_string()))?;
        let multilingual = context.is_multilingual();
        let state = context
            .create_state()
            .map_err(|e| AsrError::Load(e.to_string()))?;

        let default_threads = default_thread_count();

        tracing::info!(
            backend = %backend,
            gpu_requested = backend.is_gpu(),
            whisper_cpp = whisper_rs::WHISPER_CPP_VERSION,
            system_info = %system_info(),
            model = %model.display(),
            threads = default_threads,
            multilingual,
            load_ms = started.elapsed().as_millis() as u64,
            "asr ready"
        );

        if !backend.is_gpu() {
            tracing::warn!(
                "running whisper on the CPU — build with a GPU feature \
                 (cuda on Windows, metal on macOS) or the latency budget will not be met"
            );
        }

        Ok(Self {
            state,
            backend,
            default_threads,
            multilingual,
        })
    }
}

impl Transcriber for WhisperTranscriber {
    fn transcribe(
        &mut self,
        samples: &[f32],
        options: &TranscribeOptions,
    ) -> Result<Transcript, AsrError> {
        let audio = Duration::from_secs_f64(samples.len() as f64 / f64::from(SAMPLE_RATE));

        // An English-only model has no other language to find. Asking for
        // detection anyway costs a pass over the first window and returns a
        // meaningless answer.
        let language = if self.multilingual {
            options.language.clone()
        } else {
            Some("en".to_owned())
        };

        // Whisper pads anything shorter than 1 s to a full window anyway, and
        // an empty buffer makes it produce hallucinated boilerplate.
        if samples.is_empty() {
            return Ok(Transcript {
                text: String::new(),
                language,
                elapsed: Duration::ZERO,
                audio,
            });
        }

        // Greedy with a single candidate: beam search buys accuracy Klar does
        // not need and costs latency it cannot spare.
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_n_threads(
            options.threads.unwrap_or(self.default_threads).clamp(1, 32) as std::ffi::c_int
        );
        // Klar transcribes; it never translates. This is a product rule, not a
        // default — the polish stage is likewise forbidden from translating.
        params.set_translate(false);
        params.set_language(language.as_deref());
        // Each dictation stands alone. Carrying context across utterances is
        // how whisper starts repeating the previous sentence.
        params.set_no_context(true);
        params.set_suppress_blank(true);
        // whisper.cpp prints to stdout unless told not to; a background app
        // must not.
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        if let Some(prompt) = options.initial_prompt.as_deref()
            && !prompt.is_empty()
        {
            params.set_initial_prompt(prompt);
        }

        let started = Instant::now();
        self.state
            .full(params, samples)
            .map_err(|e| AsrError::Inference(e.to_string()))?;
        let elapsed = started.elapsed();

        let mut text = String::new();
        for index in 0..self.state.full_n_segments() {
            let Some(segment) = self.state.get_segment(index) else {
                continue;
            };
            let piece = segment
                .to_str()
                .map_err(|e| AsrError::Encoding(e.to_string()))?;
            text.push_str(piece);
        }

        // Only trust the detector when it was allowed to run.
        let detected = language.or_else(|| {
            whisper_rs::get_lang_str(self.state.full_lang_id_from_state()).map(str::to_owned)
        });

        let transcript = Transcript {
            text: text.trim().to_owned(),
            language: detected,
            elapsed,
            audio,
        };

        tracing::debug!(
            backend = %self.backend,
            audio_ms = audio.as_millis() as u64,
            elapsed_ms = elapsed.as_millis() as u64,
            rtf = transcript.real_time_factor(),
            "transcribed"
        );

        Ok(transcript)
    }

    fn backend(&self) -> Backend {
        self.backend
    }
}

/// What whisper.cpp says it was built with — the string that names the
/// accelerators actually compiled into this binary.
pub fn system_info() -> String {
    // SAFETY: whisper.cpp returns a pointer to a static, NUL-terminated buffer
    // it owns. We only read it, and do not keep the pointer.
    let raw = unsafe { whisper_rs::whisper_rs_sys::whisper_print_system_info() };
    if raw.is_null() {
        return "unavailable".to_owned();
    }
    unsafe { std::ffi::CStr::from_ptr(raw) }
        .to_string_lossy()
        .replace('|', " ")
        .trim()
        .to_owned()
}

/// Physical cores, minus a little, so a dictation does not lock up the machine
/// the user is dictating into.
fn default_thread_count() -> usize {
    let available = std::thread::available_parallelism().map_or(4, std::num::NonZero::get);
    available.saturating_sub(2).clamp(1, 8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thread_count_leaves_headroom_and_stays_in_range() {
        let threads = default_thread_count();
        assert!((1..=8).contains(&threads), "got {threads}");
    }

    /// `Result::err` drops the success value, which lets these assert on the
    /// error without `WhisperTranscriber` having to be `Debug` — it wraps an
    /// opaque C context.
    fn load_error(path: &Path) -> AsrError {
        match WhisperTranscriber::load(path).err() {
            Some(error) => error,
            None => panic!("{} unexpectedly loaded as a model", path.display()),
        }
    }

    #[test]
    fn a_missing_model_is_reported_before_anything_is_loaded() {
        let error = load_error(Path::new("/nonexistent/ggml.bin"));
        assert!(matches!(error, AsrError::ModelMissing(_)), "got {error:?}");
    }

    #[test]
    fn a_directory_is_not_mistaken_for_a_model() {
        let error = load_error(&std::env::temp_dir());
        assert!(matches!(error, AsrError::ModelMissing(_)), "got {error:?}");
    }

    #[test]
    fn system_info_names_the_build() {
        let info = system_info();
        assert!(!info.is_empty());
        // whisper.cpp always lists its SIMD/accelerator flags as `NAME = n`.
        assert!(info.contains('='), "unexpected system info: {info}");
    }
}
