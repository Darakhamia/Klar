//! A rolling log file in the app data dir, plus stderr in development.
//!
//! Nothing here leaves the machine. There is no crash reporter and no
//! telemetry — a user who needs to send us a log exports it themselves.

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

/// Install the subscriber. The returned guard flushes the file writer on drop,
/// so callers must hold it for the lifetime of the process.
pub fn init() -> Option<WorkerGuard> {
    let filter = EnvFilter::try_from_env("KLAR_LOG").unwrap_or_else(|_| EnvFilter::new("info"));

    let Some(dir) = log_dir() else {
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer())
            .init();
        tracing::warn!("no app data dir; logging to stderr only");
        return None;
    };

    if let Err(error) = std::fs::create_dir_all(&dir) {
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer())
            .init();
        tracing::warn!(%error, path = %dir.display(), "could not create log dir; stderr only");
        return None;
    }

    let appender = tracing_appender::rolling::daily(&dir, "klar.log");
    let (writer, guard) = tracing_appender::non_blocking(appender);

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer())
        .with(fmt::layer().with_ansi(false).with_writer(writer))
        .init();

    tracing::info!(path = %dir.display(), "logging to file");
    Some(guard)
}

/// Where the log file lives, and what the "export my log" button will point at.
pub fn log_dir() -> Option<std::path::PathBuf> {
    directories::ProjectDirs::from("app", "Klar", "Klar")
        .map(|dirs| dirs.data_local_dir().join("logs"))
}
