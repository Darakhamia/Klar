//! Klar's dictation pipeline.
//!
//! This crate holds everything between the microphone and the finished text and
//! knows nothing about Tauri, windows or the frontend. If a feature cannot be
//! exercised from `klar-cli`, it belongs here rather than in `src-tauri`.
//!
//! At M0 only the state machine is real; the capture, ASR, polish and storage
//! stages arrive in M1–M5.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod state;

pub use state::{IllegalTransition, Input, Machine, State, StateEvent};

/// The version reported by the CLI and the about pane.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
