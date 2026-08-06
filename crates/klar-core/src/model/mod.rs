//! The models Klar knows how to fetch, and where they live on disk.

pub mod download;

pub use download::{DownloadError, Progress, download_to};

use std::path::{Path, PathBuf};

/// What a model is for.
///
/// Klar downloads three unrelated kinds of file and they must not be offered
/// interchangeably: picking the VAD model as your speech model, or a polish
/// model as either, is a nonsense the UI should not be able to express. Until
/// now the settings window filtered the one odd entry out by comparing against
/// the literal id `silero-vad`, which worked while there was exactly one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    /// A whisper.cpp speech model. What the user chooses in onboarding.
    Speech,
    /// Voice activity detection. Required, not chosen.
    Voice,
    /// A GGUF for the polish sidecar — read by llama.cpp, not by whisper.cpp,
    /// and not interchangeable with anything above.
    Polish,
}

/// One downloadable model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct ModelSpec {
    /// What the user and the config file call it.
    pub id: &'static str,
    pub kind: Kind,
    /// Name on disk, matching upstream so a manually downloaded file drops in.
    pub file_name: &'static str,
    pub url: &'static str,
    /// SHA-256 of the finished file. Checked before the download is accepted.
    pub sha256: &'static str,
    pub bytes: u64,
    pub multilingual: bool,
    /// Shown in onboarding next to the download button.
    pub summary: &'static str,
}

impl ModelSpec {
    /// Size rounded for display: "548 MB".
    pub fn human_size(&self) -> String {
        const MB: f64 = 1024.0 * 1024.0;
        format!("{:.0} MB", self.bytes as f64 / MB)
    }
}

/// The Silero model whisper.cpp's VAD needs. Small, and required for the
/// streaming pass rather than optional.
pub const VAD_MODEL: &str = "silero-vad";

/// The default: quantised large-v3-turbo. Multilingual, and the quality/latency
/// point the 400 ms transcription budget was set against.
pub const DEFAULT_MODEL: &str = "large-v3-turbo-q5_0";

/// The default polish model.
///
/// Qwen2.5-Instruct rather than anything newer or larger, for three reasons
/// that are all about this stage in particular.
///
/// It has no reasoning mode. Every current family at this size ships one that
/// is on by default and emits its working before its answer, which against a
/// 400 ms budget is not a quality trade-off but a disqualification — and one
/// that has to be switched off through the chat template, which is a thing to
/// get wrong per model rather than never.
///
/// It is strong in more than English at 1.5B, which most models this small are
/// not. Klar polishes whatever was dictated, and a model that quietly rewrites
/// Russian into English would fail in the one way [`crate::polish::guard`]
/// cannot catch: the length and the word overlap of a translation look exactly
/// like a cleanup.
///
/// And all three sizes are one family, so they follow the same prompt the same
/// way. Somebody who moves from 1.5B to 3B for quality should get the same
/// behaviour and more of it, not a different editor.
pub const DEFAULT_POLISH_MODEL: &str = "qwen2.5-1.5b-instruct-q4";

pub const CATALOGUE: &[ModelSpec] = &[
    ModelSpec {
        id: "large-v3-turbo-q5_0",
        kind: Kind::Speech,
        file_name: "ggml-large-v3-turbo-q5_0.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin",
        sha256: "394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2",
        bytes: 574_041_195,
        multilingual: true,
        summary: "Default. Multilingual, quantised to a third of the size. Quantisation costs accuracy, and it costs it first on the words a dictionary exists for: names, jargon, anything rare.",
    },
    ModelSpec {
        id: "large-v3-turbo",
        kind: Kind::Speech,
        file_name: "ggml-large-v3-turbo.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin",
        sha256: "1fc70f774d38eb169993ac391eea357ef47c88757ef72ee5943879b7e8e2bc69",
        bytes: 1_624_555_275,
        multilingual: true,
        summary: "Unquantised. Better on unfamiliar words, three times the memory and the download. Worth it on a machine with a graphics card to spare.",
    },
    ModelSpec {
        id: "silero-vad",
        kind: Kind::Voice,
        file_name: "ggml-silero-v5.1.2.bin",
        url: "https://huggingface.co/ggml-org/whisper-vad/resolve/main/ggml-silero-v5.1.2.bin",
        sha256: "29940d98d42b91fbd05ce489f3ecf7c72f0a42f027e4875919a28fb4c04ea2cf",
        bytes: 885_098,
        multilingual: true,
        summary: "Voice activity detection. Needed for streaming; not a speech model.",
    },
    ModelSpec {
        id: "tiny.en",
        kind: Kind::Speech,
        file_name: "ggml-tiny.en.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.en.bin",
        sha256: "921e4cf8686fdd993dcd081a5da5b6c365bfde1162e72b08d75ac75289920b1f",
        bytes: 77_704_715,
        multilingual: false,
        summary: "English only, low quality. For smoke-testing the pipeline, not for use.",
    },
    ModelSpec {
        id: "qwen2.5-1.5b-instruct-q4",
        kind: Kind::Polish,
        file_name: "qwen2.5-1.5b-instruct-q4_k_m.gguf",
        url: "https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF/resolve/main/qwen2.5-1.5b-instruct-q4_k_m.gguf",
        sha256: "6a1a2eb6d15622bf3c96857206351ba97e1af16c30d7a74ee38970e434e9407e",
        bytes: 1_117_320_736,
        multilingual: true,
        summary: "Default. Cleans up dictation in the language it was spoken in, and answers inside the latency budget on a graphics card. Without one it will be slower than the budget allows and the transcript will often be used as-is.",
    },
    ModelSpec {
        id: "qwen2.5-3b-instruct-q4",
        kind: Kind::Polish,
        file_name: "qwen2.5-3b-instruct-q4_k_m.gguf",
        url: "https://huggingface.co/Qwen/Qwen2.5-3B-Instruct-GGUF/resolve/main/qwen2.5-3b-instruct-q4_k_m.gguf",
        sha256: "626b4a6678b86442240e33df819e00132d3ba7dddfe1cdc4fbb18e0a9615c62d",
        bytes: 2_104_932_768,
        multilingual: true,
        summary: "Better judgement on long sentences and self-corrections, at roughly half the speed. For a machine with a graphics card to spare.",
    },
    ModelSpec {
        id: "qwen2.5-0.5b-instruct-q4",
        kind: Kind::Polish,
        file_name: "qwen2.5-0.5b-instruct-q4_k_m.gguf",
        url: "https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF/resolve/main/qwen2.5-0.5b-instruct-q4_k_m.gguf",
        sha256: "74a4da8c9fdbcd15bd1f6d01d621410d31c6fc00986f5eb687824e7b93d7a9db",
        bytes: 491_400_032,
        multilingual: true,
        summary: "Fast enough without a graphics card, and small enough to get things wrong: at this size a model starts answering the dictation instead of tidying it. Klar refuses those, so the cost shows up as polish that silently does not happen.",
    },
];

/// The models of one kind, in the order the catalogue lists them.
pub fn of_kind(kind: Kind) -> impl Iterator<Item = &'static ModelSpec> {
    CATALOGUE.iter().filter(move |spec| spec.kind == kind)
}

/// Look a model up by id.
pub fn find(id: &str) -> Option<&'static ModelSpec> {
    CATALOGUE.iter().find(|spec| spec.id == id)
}

/// Where models are kept: alongside the app's data, not in the install dir, so
/// an update never has to re-download half a gigabyte.
pub fn models_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("app", "Klar", "Klar")
        .map(|dirs| dirs.data_local_dir().join("models"))
}

/// Full path a model would occupy inside `dir`.
pub fn path_in(dir: &Path, spec: &ModelSpec) -> PathBuf {
    dir.join(spec.file_name)
}

/// Whether a model looks present. Size only — a full hash of half a gigabyte on
/// every launch would be felt. [`download::verify`] does the real check.
pub fn is_present(dir: &Path, spec: &ModelSpec) -> bool {
    std::fs::metadata(path_in(dir, spec)).is_ok_and(|m| m.len() == spec.bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_model_is_in_the_catalogue() {
        assert!(find(DEFAULT_MODEL).is_some());
    }

    #[test]
    fn the_default_polish_model_is_in_the_catalogue_and_is_a_polish_model() {
        let spec = find(DEFAULT_POLISH_MODEL).expect("the default polish model exists");
        assert_eq!(spec.kind, Kind::Polish);
    }

    /// The settings window offers whatever `of_kind(Speech)` returns. A polish
    /// model reaching that list would be offered as something to transcribe
    /// with, and whisper.cpp would fail to load a GGUF in a way that reads as
    /// a broken install.
    #[test]
    fn the_kinds_do_not_leak_into_each_others_lists() {
        for spec in of_kind(Kind::Speech) {
            assert_eq!(spec.kind, Kind::Speech, "{}", spec.id);
            assert!(
                spec.file_name.ends_with(".bin"),
                "{}: a speech model is a ggml .bin",
                spec.id
            );
        }
        for spec in of_kind(Kind::Polish) {
            assert!(
                spec.file_name.ends_with(".gguf"),
                "{}: a polish model is a llama.cpp .gguf",
                spec.id
            );
        }
        assert_eq!(of_kind(Kind::Voice).count(), 1, "there is one VAD model");
        assert_eq!(
            of_kind(Kind::Speech).count()
                + of_kind(Kind::Voice).count()
                + of_kind(Kind::Polish).count(),
            CATALOGUE.len(),
            "every entry must be one of the three kinds"
        );
    }

    #[test]
    fn ids_and_file_names_are_unique() {
        for (i, a) in CATALOGUE.iter().enumerate() {
            for b in &CATALOGUE[i + 1..] {
                assert_ne!(a.id, b.id, "duplicate id {}", a.id);
                assert_ne!(
                    a.file_name, b.file_name,
                    "duplicate file name {}",
                    a.file_name
                );
            }
        }
    }

    #[test]
    fn every_entry_carries_a_full_sha256() {
        for spec in CATALOGUE {
            assert_eq!(spec.sha256.len(), 64, "{}: not a sha256", spec.id);
            assert!(
                spec.sha256
                    .chars()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
                "{}: sha must be lowercase hex",
                spec.id
            );
            assert!(spec.bytes > 0, "{}: no size", spec.id);
            assert!(
                spec.url.starts_with("https://"),
                "{}: model urls must be https",
                spec.id
            );
        }
    }

    #[test]
    fn unknown_ids_return_none_rather_than_a_default() {
        assert!(find("large-v4").is_none());
        assert!(find("").is_none());
    }

    #[test]
    fn size_is_rendered_for_humans() {
        let spec = find(DEFAULT_MODEL).unwrap();
        assert_eq!(spec.human_size(), "547 MB");
    }

    #[test]
    fn presence_needs_the_exact_size() {
        let dir = std::env::temp_dir().join("klar-model-presence");
        std::fs::create_dir_all(&dir).unwrap();
        let spec = find("tiny.en").unwrap();

        assert!(!is_present(&dir, spec));

        std::fs::write(path_in(&dir, spec), b"truncated").unwrap();
        assert!(
            !is_present(&dir, spec),
            "a short file must not count as present"
        );

        std::fs::remove_dir_all(&dir).ok();
    }
}
