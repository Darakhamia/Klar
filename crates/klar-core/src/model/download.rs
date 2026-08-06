//! Fetching a model, resumably, and refusing to accept a corrupt one.
//!
//! Models are half a gigabyte on a connection that may drop, so the download
//! goes to a `.part` file and continues with a `Range` request if one is
//! already there. The finished file is hashed before it is moved into place: a
//! truncated model does not fail loudly at load time, it fails strangely at
//! inference time, which is far worse to diagnose.

use super::ModelSpec;
use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("network: {0}")]
    Network(String),

    #[error("the server does not support resuming; delete {0} and start again")]
    NoResume(PathBuf),

    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error(
        "the downloaded file does not match the expected checksum \
         (expected {expected}, got {actual}) — the file was not installed"
    )]
    Checksum { expected: String, actual: String },

    #[error("the server sent {actual} bytes, expected {expected}")]
    Size { expected: u64, actual: u64 },
}

/// Where the download has got to. Reported often enough for a progress bar and
/// no more.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    /// Bytes already on disk from an earlier attempt.
    Resuming {
        from: u64,
        total: u64,
    },
    Downloading {
        received: u64,
        total: u64,
    },
    /// Hashing the finished file. On a slow disk this is not instant, so it
    /// gets its own state rather than looking like a hang at 100%.
    Verifying,
    Done,
}

/// Download `spec` into `dir`, resuming if a partial file is there.
///
/// Returns the path to the installed model. A model already present and the
/// right size is left alone.
pub async fn download_to(
    spec: &ModelSpec,
    dir: &Path,
    mut on_progress: impl FnMut(Progress),
) -> Result<PathBuf, DownloadError> {
    let final_path = dir.join(spec.file_name);
    let part_path = dir.join(format!("{}.part", spec.file_name));

    if super::is_present(dir, spec) {
        on_progress(Progress::Done);
        return Ok(final_path);
    }

    std::fs::create_dir_all(dir).map_err(|source| DownloadError::Io {
        path: dir.to_path_buf(),
        source,
    })?;

    let already = std::fs::metadata(&part_path).map(|m| m.len()).unwrap_or(0);
    // A `.part` at or past the full size is not a resume point, it is junk from
    // an interrupted run against a different file.
    let already = if already >= spec.bytes { 0 } else { already };

    let client = reqwest::Client::builder()
        .user_agent(concat!("klar/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| DownloadError::Network(e.to_string()))?;

    let mut request = client.get(spec.url);
    if already > 0 {
        request = request.header(reqwest::header::RANGE, format!("bytes={already}-"));
        on_progress(Progress::Resuming {
            from: already,
            total: spec.bytes,
        });
    }

    let response = request
        .send()
        .await
        .map_err(|e| DownloadError::Network(e.to_string()))?;
    let status = response.status();
    if !status.is_success() {
        return Err(DownloadError::Network(format!(
            "{} {}",
            status.as_u16(),
            status
        )));
    }

    // We asked to resume and the server ignored it: starting over silently
    // would append to the partial file and produce a corrupt model.
    let resumed = status == reqwest::StatusCode::PARTIAL_CONTENT;
    if already > 0 && !resumed {
        return Err(DownloadError::NoResume(part_path));
    }

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(!resumed)
        .open(&part_path)
        .map_err(|source| DownloadError::Io {
            path: part_path.clone(),
            source,
        })?;

    if resumed {
        file.seek(SeekFrom::Start(already))
            .map_err(|source| DownloadError::Io {
                path: part_path.clone(),
                source,
            })?;
    }

    let mut written = already;
    let mut stream = response.bytes_stream();
    let mut since_report = 0_u64;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| DownloadError::Network(e.to_string()))?;
        file.write_all(&chunk).map_err(|source| DownloadError::Io {
            path: part_path.clone(),
            source,
        })?;
        written += chunk.len() as u64;

        since_report += chunk.len() as u64;
        if since_report >= 1024 * 1024 {
            since_report = 0;
            on_progress(Progress::Downloading {
                received: written,
                total: spec.bytes,
            });
        }
    }

    file.flush().map_err(|source| DownloadError::Io {
        path: part_path.clone(),
        source,
    })?;
    drop(file);

    on_progress(Progress::Downloading {
        received: written,
        total: spec.bytes,
    });

    if written != spec.bytes {
        return Err(DownloadError::Size {
            expected: spec.bytes,
            actual: written,
        });
    }

    on_progress(Progress::Verifying);
    let actual = sha256_of(&part_path)?;
    if actual != spec.sha256 {
        // Leave the bad file where it is rather than moving it into place; the
        // caller decides whether to retry or delete.
        return Err(DownloadError::Checksum {
            expected: spec.sha256.to_owned(),
            actual,
        });
    }

    std::fs::rename(&part_path, &final_path).map_err(|source| DownloadError::Io {
        path: final_path.clone(),
        source,
    })?;

    on_progress(Progress::Done);
    Ok(final_path)
}

/// Hash an installed model and compare it against the catalogue. Slow by
/// design — this is the explicit check, not something on the launch path.
pub fn verify(spec: &ModelSpec, dir: &Path) -> Result<bool, DownloadError> {
    let path = dir.join(spec.file_name);
    Ok(sha256_of(&path)? == spec.sha256)
}

fn sha256_of(path: &Path) -> Result<String, DownloadError> {
    use std::io::Read;

    let mut file = std::fs::File::open(path).map_err(|source| DownloadError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut hasher = Sha256::new();
    // Read in chunks rather than slurping: these files are hundreds of
    // megabytes and this runs on a machine that also has to stay under 40 MB.
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|source| DownloadError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::net::TcpListener;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("klar-dl-{name}"));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// The body every test model download serves.
    fn body() -> Vec<u8> {
        (0..40_000_u32).map(|i| (i % 251) as u8).collect()
    }

    fn sha_of(bytes: &[u8]) -> String {
        hex::encode(Sha256::digest(bytes))
    }

    /// A single-request HTTP/1.1 server that understands `Range: bytes=N-`.
    /// `honour_range` off simulates a CDN that ignores the header — the case
    /// that would otherwise corrupt a resumed file.
    fn serve_once(body: Vec<u8>, honour_range: bool) -> (String, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        let handle = std::thread::spawn(move || {
            let Ok((stream, _)) = listener.accept() else {
                return;
            };
            let mut reader = BufReader::new(&stream);
            let mut range_from = 0_u64;

            loop {
                let mut line = String::new();
                // Not `unwrap_or(0)`: swallowing a read error here would end
                // the header loop early, and the server would answer 200
                // without ever having seen the Range header — which the resume
                // test then reports as the downloader failing to resume. A
                // broken test server must look broken.
                match reader.read_line(&mut line) {
                    Ok(0) => break,
                    Ok(_) if line == "\r\n" => break,
                    Ok(_) => {}
                    Err(error) => panic!("test server could not read the request: {error}"),
                }
                if let Some(rest) = line.to_ascii_lowercase().strip_prefix("range: bytes=") {
                    range_from = rest.trim().trim_end_matches('-').parse().unwrap_or(0);
                }
            }

            let mut out = &stream;
            if range_from > 0 && honour_range {
                let slice = &body[range_from as usize..];
                let header = format!(
                    "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\n\
                     Content-Range: bytes {}-{}/{}\r\nConnection: close\r\n\r\n",
                    slice.len(),
                    range_from,
                    body.len() - 1,
                    body.len()
                );
                out.write_all(header.as_bytes()).unwrap();
                out.write_all(slice).unwrap();
            } else {
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                out.write_all(header.as_bytes()).unwrap();
                out.write_all(&body).unwrap();
            }
            out.flush().ok();
            drop(stream);
        });

        (format!("http://127.0.0.1:{port}/model.bin"), handle)
    }

    /// Build a spec pointing at the test server. The fields the downloader
    /// reads are `url`, `bytes`, `sha256` and `file_name`.
    fn spec_for(url: &str, body: &[u8], sha: &str) -> ModelSpec {
        ModelSpec {
            kind: crate::model::Kind::Speech,
            id: "test",
            file_name: "test-model.bin",
            url: Box::leak(url.to_owned().into_boxed_str()),
            sha256: Box::leak(sha.to_owned().into_boxed_str()),
            bytes: body.len() as u64,
            multilingual: false,
            summary: "",
        }
    }

    #[tokio::test]
    async fn a_clean_download_verifies_and_installs() {
        let dir = temp_dir("clean");
        let body = body();
        let (url, server) = serve_once(body.clone(), true);
        let spec = spec_for(&url, &body, &sha_of(&body));

        let mut states = Vec::new();
        let path = download_to(&spec, &dir, |p| states.push(p)).await.unwrap();
        server.join().unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), body);
        assert!(states.contains(&Progress::Verifying));
        assert_eq!(states.last(), Some(&Progress::Done));
        // The scratch file must not be left behind.
        assert!(!dir.join("test-model.bin.part").exists());
    }

    #[tokio::test]
    async fn a_partial_file_resumes_instead_of_restarting() {
        let dir = temp_dir("resume");
        let body = body();
        // Half a download from an earlier, interrupted run.
        let head = &body[..15_000];
        std::fs::write(dir.join("test-model.bin.part"), head).unwrap();

        let (url, server) = serve_once(body.clone(), true);
        let spec = spec_for(&url, &body, &sha_of(&body));

        let mut states = Vec::new();
        let path = download_to(&spec, &dir, |p| states.push(p)).await.unwrap();
        server.join().unwrap();

        assert_eq!(
            std::fs::read(&path).unwrap(),
            body,
            "resumed file must match byte for byte"
        );
        assert!(
            states.contains(&Progress::Resuming {
                from: 15_000,
                total: body.len() as u64
            }),
            "expected a resume, got {states:?}"
        );
    }

    #[tokio::test]
    async fn a_server_that_ignores_range_is_refused_not_appended_to() {
        let dir = temp_dir("norange");
        let body = body();
        std::fs::write(dir.join("test-model.bin.part"), &body[..15_000]).unwrap();

        let (url, server) = serve_once(body.clone(), false);
        let spec = spec_for(&url, &body, &sha_of(&body));

        let result = download_to(&spec, &dir, |_| {}).await;
        server.join().unwrap();

        assert!(
            matches!(result, Err(DownloadError::NoResume(_))),
            "got {result:?}"
        );
        assert!(
            !dir.join("test-model.bin").exists(),
            "nothing may be installed"
        );
    }

    #[tokio::test]
    async fn a_corrupt_download_is_not_installed() {
        let dir = temp_dir("corrupt");
        let body = body();
        let (url, server) = serve_once(body.clone(), true);
        // The catalogue expects something else entirely.
        let spec = spec_for(&url, &body, &sha_of(b"different content"));

        let result = download_to(&spec, &dir, |_| {}).await;
        server.join().unwrap();

        assert!(
            matches!(result, Err(DownloadError::Checksum { .. })),
            "got {result:?}"
        );
        assert!(
            !dir.join("test-model.bin").exists(),
            "a bad model must never be installed"
        );
    }

    #[tokio::test]
    async fn an_oversized_part_file_starts_over_rather_than_resuming_past_the_end() {
        let dir = temp_dir("oversized");
        let body = body();
        // Junk from an interrupted run against a different, larger file.
        std::fs::write(
            dir.join("test-model.bin.part"),
            vec![0_u8; body.len() + 500],
        )
        .unwrap();

        let (url, server) = serve_once(body.clone(), true);
        let spec = spec_for(&url, &body, &sha_of(&body));

        let path = download_to(&spec, &dir, |_| {}).await.unwrap();
        server.join().unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), body);
    }

    #[tokio::test]
    async fn an_installed_model_is_not_downloaded_again() {
        let dir = temp_dir("present");
        let body = body();
        let spec = spec_for("http://127.0.0.1:1/never", &body, &sha_of(&body));
        std::fs::write(dir.join("test-model.bin"), &body).unwrap();

        // No server is listening; reaching the network at all would fail.
        let path = download_to(&spec, &dir, |_| {}).await.unwrap();
        assert_eq!(path, dir.join("test-model.bin"));
    }

    #[test]
    fn verify_hashes_what_is_actually_on_disk() {
        let dir = temp_dir("verify");
        let body = body();
        let spec = spec_for("http://unused", &body, &sha_of(&body));

        std::fs::write(dir.join("test-model.bin"), &body).unwrap();
        assert!(verify(&spec, &dir).unwrap());

        std::fs::write(dir.join("test-model.bin"), b"tampered").unwrap();
        assert!(!verify(&spec, &dir).unwrap());
    }

    #[test]
    fn verifying_a_missing_file_errors_rather_than_reporting_a_mismatch() {
        let dir = temp_dir("missing");
        let spec = spec_for("http://unused", b"", &sha_of(b""));
        assert!(matches!(verify(&spec, &dir), Err(DownloadError::Io { .. })));
    }

    #[test]
    fn sha256_matches_a_known_vector() {
        let dir = temp_dir("known");
        let path = dir.join("abc.txt");
        std::fs::write(&path, b"abc").unwrap();
        assert_eq!(
            sha256_of(&path).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
