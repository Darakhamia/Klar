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

/// The default polish model: the smallest one measured that does the job.
///
/// Every entry below was run against `tests/polish_fixtures.rs`, which is the
/// only reason any of this is asserted rather than assumed.
///
/// | Model | Fixtures | Where it fails |
/// |---|---|---|
/// | Qwen3-4B-Instruct-2507 | 4 of 6 | self-correction, code-switching |
/// | Qwen2.5-3B | worse than 1.5B | truncates, mixes scripts — dropped |
/// | Qwen2.5-1.5B | 3 of 6 | above, plus heavy leaves the padding in |
///
/// The `-Instruct-2507` suffix matters and is not decoration: plain Qwen3 has
/// a reasoning mode that is on by default and writes its working before its
/// answer, which against a budget in hundreds of milliseconds is not a quality
/// trade-off but a disqualification. The 2507 instruct release has no such
/// mode to switch off, so there is nothing to get wrong per model.
///
/// Being strong in more than English is a requirement rather than a bonus.
/// Klar polishes whatever was dictated, and a model that rewrites Russian into
/// English fails in a way that reads as Klar being broken.
///
/// Bigger is not the axis. 3B measured worse than the 1.5B it was meant to
/// improve on — it dropped the first half of sentences — which is why the
/// remaining three are two sizes of one family plus the one that beat both.
pub const DEFAULT_POLISH_MODEL: &str = "qwen3-4b-instruct-q4";

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
        id: "qwen3-4b-instruct-q4",
        kind: Kind::Polish,
        file_name: "qwen3-4b-instruct-2507-q4_k_m.gguf",
        url: "https://huggingface.co/bartowski/Qwen_Qwen3-4B-Instruct-2507-GGUF/resolve/main/Qwen_Qwen3-4B-Instruct-2507-Q4_K_M.gguf",
        sha256: "2fde00ce69dd4899c70d020845e2638353015bba0fdf161b3eb965f2bca4464e",
        bytes: 2_497_280_736,
        multilingual: true,
        summary: "Default. The smallest model measured that cleans up dictation properly in more than English. Needs a graphics card to answer in time; without one it will miss the budget and the transcript will be used as it was heard.",
    },
    ModelSpec {
        id: "qwen2.5-1.5b-instruct-q4",
        kind: Kind::Polish,
        file_name: "qwen2.5-1.5b-instruct-q4_k_m.gguf",
        url: "https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF/resolve/main/qwen2.5-1.5b-instruct-q4_k_m.gguf",
        sha256: "6a1a2eb6d15622bf3c96857206351ba97e1af16c30d7a74ee38970e434e9407e",
        bytes: 1_117_320_736,
        multilingual: true,
        summary: "Half the size and roughly three times the speed of the default, and it shows: it punctuates well but leaves self-corrections unresolved. For a machine that cannot keep the default inside the budget.",
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
