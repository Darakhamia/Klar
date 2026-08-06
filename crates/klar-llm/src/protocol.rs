//! What Klar and the sidecar say to each other.
//!
//! One JSON object per line, in both directions, over the child's stdin and
//! stdout. Deliberately not HTTP: an HTTP sidecar means a port open on
//! localhost for as long as Klar is running, which every other process on the
//! machine can reach, and Klar's whole claim is that dictated text does not
//! leave the pipeline. A pipe is reachable by the parent and nobody else.
//!
//! Line-delimited rather than length-prefixed because the sidecar's other
//! output — llama.cpp's own logging — goes to stderr, and a protocol a person
//! can read while debugging is worth more here than a few saved bytes.
//!
//! This module is shared: `klar-core` depends on this crate for these types, so
//! the two ends cannot disagree about the shape. It pulls in no llama.cpp — see
//! the note in `lib.rs`.

use serde::{Deserialize, Serialize};

/// One polish job.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    /// Echoed back on the answer. The protocol is one-at-a-time today, and
    /// this is what stops a late reply to an abandoned request being read as
    /// the answer to the next one.
    pub id: u64,
    pub system: String,
    pub user: String,
    /// A hard ceiling on the reply, in tokens. A model that has started
    /// repeating itself has to be stopped by something.
    pub max_tokens: u32,
}

/// What the sidecar says back.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Response {
    /// Sent once, when the model is loaded and the first request will not pay
    /// for loading it.
    Ready {
        model: String,
        /// What llama.cpp reports it is running on, so a machine quietly on the
        /// CPU can be told so rather than just being slow.
        backend: String,
        load_ms: u64,
    },
    Done {
        id: u64,
        text: String,
        elapsed_ms: u64,
        tokens: u32,
    },
    Failed {
        id: u64,
        message: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The two ends are separate processes that can be different builds during
    /// development. A round trip through the wire format is the only check
    /// that they still agree.
    #[test]
    fn a_request_survives_the_wire() {
        let request = Request {
            id: 7,
            system: "Tidy this up.".to_owned(),
            user: "so um I guess we should push it".to_owned(),
            max_tokens: 256,
        };
        let line = serde_json::to_string(&request).unwrap();
        assert!(
            !line.contains('\n'),
            "a request must fit on one line: {line}"
        );

        let back: Request = serde_json::from_str(&line).unwrap();
        assert_eq!(back.id, 7);
        assert_eq!(back.user, request.user);
    }

    /// Dictated text contains newlines, quotes and emoji, and all three would
    /// break a protocol that split on anything but a JSON boundary.
    #[test]
    fn awkward_text_does_not_break_the_framing() {
        let request = Request {
            id: 1,
            system: String::new(),
            user: "line one\nline \"two\" — 🎤\r\nline three".to_owned(),
            max_tokens: 8,
        };
        let line = serde_json::to_string(&request).unwrap();
        assert_eq!(line.lines().count(), 1);
        let back: Request = serde_json::from_str(&line).unwrap();
        assert_eq!(back.user, request.user);
    }

    #[test]
    fn responses_are_told_apart_by_kind() {
        let done = Response::Done {
            id: 3,
            text: "Tidy.".to_owned(),
            elapsed_ms: 120,
            tokens: 4,
        };
        let line = serde_json::to_string(&done).unwrap();
        assert!(line.contains("\"kind\":\"done\""), "{line}");

        match serde_json::from_str::<Response>(&line).unwrap() {
            Response::Done { id, text, .. } => {
                assert_eq!(id, 3);
                assert_eq!(text, "Tidy.");
            }
            other => panic!("round-tripped into {other:?}"),
        }
    }
}
