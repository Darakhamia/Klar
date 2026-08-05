//! Build-time work for the Tauri wrapper.
//!
//! Beyond Tauri's own codegen there is one job: collecting the CUDA runtime
//! libraries the bundled application will need, so the installer can carry them.

use std::path::{Path, PathBuf};

/// Where the collected libraries go. Listed in `tauri.conf.json` as a resource
/// mapped to the install directory, which on Windows is where the executable
/// looks for its DLLs. Gitignored — these are NVIDIA's files, not ours.
const COLLECTED: &str = "cuda-runtime";

/// The libraries whisper.cpp's CUDA backend links against.
///
/// Prefixes rather than names: the suffix is the CUDA major version, and
/// hard-coding one would break silently on the next toolkit. Whatever is in
/// this machine's toolkit is what the binary was linked against, so whatever is
/// there is what ships.
const NEEDED: &[&str] = &["cudart64_", "cublas64_", "cublasLt64_"];

fn main() {
    // Always, even on a CPU build: `tauri.conf.json` names this directory as a
    // resource, and a glob whose parent does not exist is a bundler error
    // rather than an empty match.
    if let Err(error) = std::fs::create_dir_all(COLLECTED) {
        println!("cargo:warning=could not create {COLLECTED}: {error}");
    }

    if std::env::var_os("CARGO_FEATURE_CUDA").is_some() {
        collect_cuda_runtime();
    }

    tauri_build::build();
}

/// Copy the CUDA runtime next to what the bundler will package.
///
/// whisper.cpp links these dynamically, so a machine without the CUDA Toolkit
/// cannot start Klar at all — it fails at load time with a missing-DLL error,
/// before any of our code runs and before anything can explain itself. The
/// toolkit is a multi-gigabyte developer download; the three libraries the
/// application actually needs are a fraction of it, and NVIDIA permits
/// redistributing them with an application.
///
/// Failure here is a warning rather than an error. A developer build runs on a
/// machine that has the toolkit on `PATH` and needs none of this; only the
/// installer does.
fn collect_cuda_runtime() {
    let Some(bin) = cuda_bin() else {
        println!(
            "cargo:warning=CUDA_PATH is not set, so the CUDA runtime will not be \
             bundled. This build will run here; an installer made from it will not \
             start on a machine without the CUDA Toolkit."
        );
        return;
    };

    let into = Path::new(COLLECTED);
    let entries = match std::fs::read_dir(&bin) {
        Ok(entries) => entries,
        Err(error) => {
            println!("cargo:warning=could not read {}: {error}", bin.display());
            return;
        }
    };

    let mut copied = 0;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };

        if !name.ends_with(".dll") || !NEEDED.iter().any(|prefix| name.starts_with(prefix)) {
            continue;
        }

        match std::fs::copy(entry.path(), into.join(name)) {
            Ok(_) => copied += 1,
            Err(error) => println!("cargo:warning=could not copy {name}: {error}"),
        }
    }

    if copied == 0 {
        println!(
            "cargo:warning=no CUDA runtime libraries found in {} — an installer \
             made from this build will not start without the CUDA Toolkit.",
            bin.display()
        );
    }

    println!("cargo:rerun-if-changed={}", bin.display());
}

fn cuda_bin() -> Option<PathBuf> {
    let root = std::env::var_os("CUDA_PATH")?;
    let bin = PathBuf::from(root).join("bin");
    bin.is_dir().then_some(bin)
}
