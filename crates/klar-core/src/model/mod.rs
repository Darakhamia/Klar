//! The whisper models Klar knows how to fetch, and where they live on disk.

pub mod download;

pub use download::{DownloadError, Progress, download_to};

use std::path::{Path, PathBuf};

/// One downloadable ggml model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct ModelSpec {
    /// What the user and the config file call it.
    pub id: &'static str,
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

/// The default: quantised large-v3-turbo. Multilingual, and the quality/latency
/// point the 400 ms transcription budget was set against.
pub const DEFAULT_MODEL: &str = "large-v3-turbo-q5_0";

pub const CATALOGUE: &[ModelSpec] = &[
    ModelSpec {
        id: "large-v3-turbo-q5_0",
        file_name: "ggml-large-v3-turbo-q5_0.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin",
        sha256: "394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2",
        bytes: 574_041_195,
        multilingual: true,
        summary: "Default. Multilingual, quantised — the quality Klar was tuned for.",
    },
    ModelSpec {
        id: "large-v3-turbo",
        file_name: "ggml-large-v3-turbo.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin",
        sha256: "1fc70f774d38eb169993ac391eea357ef47c88757ef72ee5943879b7e8e2bc69",
        bytes: 1_624_555_275,
        multilingual: true,
        summary: "Unquantised. Marginally better, three times the memory.",
    },
    ModelSpec {
        id: "tiny.en",
        file_name: "ggml-tiny.en.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.en.bin",
        sha256: "921e4cf8686fdd993dcd081a5da5b6c365bfde1162e72b08d75ac75289920b1f",
        bytes: 77_704_715,
        multilingual: false,
        summary: "English only, low quality. For smoke-testing the pipeline, not for use.",
    },
];

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
