//! Speech to text.
//!
//! One trait, so the pipeline never names whisper directly, and so M3 can swap
//! the one-shot implementation here for a streaming one without touching
//! anything above it.

pub mod devices;
pub mod whisper;

pub use devices::{Acceleration, Device, DeviceKind};
pub use whisper::WhisperTranscriber;

use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum AsrError {
    #[error("model not found at {0}")]
    ModelMissing(std::path::PathBuf),

    #[error("could not load the model: {0}")]
    Load(String),

    #[error("transcription failed: {0}")]
    Inference(String),

    #[error("the transcript was not valid UTF-8: {0}")]
    Encoding(String),
}

/// Which compute backend whisper.cpp was built against.
///
/// This is what the build asked for, and it is only half the answer: a CUDA
/// build on a machine with an AMD card is still `Cuda` here while every
/// transcription runs on the CPU. [`Acceleration::probe`] asks ggml what it
/// actually found, and that is the one to show a user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Backend {
    Cpu,
    Cuda,
    Vulkan,
    Metal,
    CoreMl,
}

impl Backend {
    /// The backend this binary was compiled with. Exactly one GPU feature is
    /// expected; if several are on, the first here wins and the mismatch is
    /// worth noticing in the log.
    pub const fn compiled() -> Self {
        if cfg!(feature = "cuda") {
            Self::Cuda
        } else if cfg!(feature = "metal") {
            Self::Metal
        } else if cfg!(feature = "coreml") {
            Self::CoreMl
        } else if cfg!(feature = "vulkan") {
            Self::Vulkan
        } else {
            Self::Cpu
        }
    }

    pub const fn is_gpu(&self) -> bool {
        !matches!(self, Self::Cpu)
    }

    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Cuda => "cuda",
            Self::Vulkan => "vulkan",
            Self::Metal => "metal",
            Self::CoreMl => "coreml",
        }
    }
}

impl std::fmt::Display for Backend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.pad(self.as_str())
    }
}

/// Per-utterance settings.
#[derive(Debug, Clone, Default)]
pub struct TranscribeOptions {
    /// ISO code, or `None` to let whisper detect. Detection costs a pass over
    /// the first window, so the app pins this once the user has chosen.
    pub language: Option<String>,

    /// Biases recognition toward terms the user has taught Klar. Wired up in
    /// M5 when the dictionary exists.
    pub initial_prompt: Option<String>,

    /// `None` lets the implementation pick from the core count.
    pub threads: Option<usize>,
}

/// What came back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transcript {
    /// The recognised text, trimmed. Never contains whisper's own markers.
    pub text: String,
    /// Detected or configured language, if whisper reported one.
    pub language: Option<String>,
    /// Wall-clock time inference took. The number M1 and M3 are measured on.
    pub elapsed: Duration,
    /// How much audio went in, for the real-time factor in the log.
    pub audio: Duration,
}

impl Transcript {
    /// Seconds of audio processed per second of wall clock. Above 1.0 means
    /// faster than real time, which the streaming pass in M3 depends on.
    pub fn real_time_factor(&self) -> f64 {
        let elapsed = self.elapsed.as_secs_f64();
        if elapsed <= 0.0 {
            f64::INFINITY
        } else {
            self.audio.as_secs_f64() / elapsed
        }
    }
}

pub trait Transcriber: Send {
    fn transcribe(
        &mut self,
        samples: &[f32],
        options: &TranscribeOptions,
    ) -> Result<Transcript, AsrError>;

    /// The backend this transcriber was built against.
    fn backend(&self) -> Backend;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_is_not_a_gpu_backend() {
        assert!(!Backend::Cpu.is_gpu());
        assert!(Backend::Cuda.is_gpu());
        assert!(Backend::Metal.is_gpu());
    }

    #[test]
    fn the_compiled_backend_matches_the_enabled_feature() {
        let expected = if cfg!(feature = "cuda") {
            Backend::Cuda
        } else if cfg!(feature = "metal") {
            Backend::Metal
        } else if cfg!(feature = "coreml") {
            Backend::CoreMl
        } else if cfg!(feature = "vulkan") {
            Backend::Vulkan
        } else {
            Backend::Cpu
        };
        assert_eq!(Backend::compiled(), expected);
    }

    #[test]
    fn real_time_factor_is_audio_over_wall_clock() {
        let transcript = Transcript {
            text: String::new(),
            language: None,
            elapsed: Duration::from_millis(500),
            audio: Duration::from_secs(10),
        };
        assert!((transcript.real_time_factor() - 20.0).abs() < 1e-9);
    }

    #[test]
    fn a_zero_length_run_does_not_divide_by_zero() {
        let transcript = Transcript {
            text: String::new(),
            language: None,
            elapsed: Duration::ZERO,
            audio: Duration::from_secs(1),
        };
        assert!(transcript.real_time_factor().is_infinite());
    }
}
