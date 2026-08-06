//! The polish model, as a process of its own.
//!
//! # Why this is not a module in `klar-core`
//!
//! Klar already links whisper.cpp, and whisper.cpp statically links ggml.
//! llama.cpp statically links ggml as well. Putting both in one binary was
//! tried, on Linux, before any of this was written, and the result is worth
//! recording because it does not look like a failure:
//!
//! - It **links**. No duplicate-symbol error, no warning.
//! - The two archives export 644 identical `ggml_*` symbols, and llama.cpp's
//!   copy exports 39 the whisper.cpp one does not. They are different versions
//!   of ggml.
//! - The linker silently resolves each symbol to whichever archive it reaches
//!   first, mixing object files from two versions of the same library into one
//!   address space.
//!
//! A link error would have been the good outcome: it stops you. This ships.
//! Every future bump of either crate reshuffles which version wins, with no
//! error to notice and nothing in the test suite that would catch a struct
//! that grew a field between the two.
//!
//! So the model runs in a child process. That is not a workaround with a cost
//! attached; it is better in three ways that matter here:
//!
//! - **No shared symbols at all.** The question stops existing rather than
//!   being managed.
//! - **A crash cannot take Klar down.** `CLAUDE.md` forbids a panic on any path
//!   reachable from audio or hotkey handling, because a crash there kills a
//!   background app the user cannot see. A model that runs out of VRAM kills
//!   the child; the hotkey keeps working and the transcript is inserted
//!   unpolished, which is exactly the fallback the polish stage already has.
//! - **No port.** The obvious sidecar is an HTTP server on localhost, and that
//!   is a socket every other process on the machine can reach for as long as
//!   Klar is running. A pipe is reachable by the parent and nobody else.
//!
//! # What is in this crate
//!
//! With no features: [`protocol`], and nothing else. `klar-core` depends on it
//! that way so the two ends cannot disagree about the wire format, and so that
//! building or testing the library does not build llama.cpp.
//!
//! With `--features engine`: the `klar-llm` binary, which is the only thing
//! here that touches llama.cpp.

// Tests may unwrap and panic; the crate's own code may not. Same line as
// klar-core's, for the same reason: a failed assertion is how a test reports.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod protocol;
