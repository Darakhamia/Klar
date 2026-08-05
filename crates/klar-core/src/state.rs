//! The dictation state machine.
//!
//! `Idle → Recording → Transcribing → Polishing → Injecting → Idle`, plus
//! `Error` reachable from any state. Every accepted transition emits a
//! [`StateEvent`]; the overlay renders purely from these and holds no state of
//! its own.

use serde::Serialize;
use std::fmt;

/// Where the pipeline currently is. The overlay maps these one-to-one onto its
/// five visual states — `Transcribing` is "thinking", `Injecting` folds into
/// "done".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum State {
    Idle,
    Recording,
    Transcribing,
    Polishing,
    Injecting,
    Error,
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Idle => "idle",
            Self::Recording => "recording",
            Self::Transcribing => "transcribing",
            Self::Polishing => "polishing",
            Self::Injecting => "injecting",
            Self::Error => "error",
        };
        f.pad(name)
    }
}

/// What the pipeline reports as it moves. Serialised straight to the frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "data")]
pub enum StateEvent {
    /// The machine entered a new state.
    Entered(State),
    /// A partial transcript from the sliding window, shown while recording.
    Partial(String),
    /// The finished text, immediately before injection.
    Final(String),
    /// Something failed. Carries a message written for a user, not a log.
    Failed(String),
}

/// Inputs the machine accepts. Anything not listed as a legal transition below
/// is rejected rather than silently applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    /// Hotkey went down.
    Start,
    /// Hotkey came up; the tail is still being processed.
    Stop,
    /// A partial transcript arrived from the sliding window.
    Partial(String),
    /// Final transcription is done, text is ready for polish.
    Transcribed(String),
    /// Polish is done, text is ready to inject.
    Polished(String),
    /// The text reached the focused application.
    Injected,
    /// Anything went wrong, at any point.
    Fail(String),
    /// The user acknowledged an error, or it timed out.
    Dismiss,
}

/// A transition the machine refused, kept as a value so callers can log it
/// instead of panicking on an unexpected event ordering.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("input {input} is not legal in state {state}")]
pub struct IllegalTransition {
    pub state: State,
    pub input: &'static str,
}

/// The machine itself. Holds the current state and nothing else — the audio,
/// the transcript and the polished text live with the stages that own them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Machine {
    state: State,
}

impl Default for Machine {
    fn default() -> Self {
        Self::new()
    }
}

impl Machine {
    pub const fn new() -> Self {
        Self { state: State::Idle }
    }

    pub const fn state(&self) -> State {
        self.state
    }

    /// Apply an input. On success returns the events the caller should emit, in
    /// order. On failure the state is left untouched.
    pub fn apply(&mut self, input: Input) -> Result<Vec<StateEvent>, IllegalTransition> {
        use Input as I;
        use State as S;

        // A failure is legal from anywhere except Idle, where there is nothing
        // running to fail.
        if let I::Fail(message) = input {
            return if self.state == S::Idle {
                Err(self.illegal("Fail"))
            } else {
                self.state = S::Error;
                Ok(vec![
                    StateEvent::Entered(S::Error),
                    StateEvent::Failed(message),
                ])
            };
        }

        match (self.state, input) {
            (S::Idle, I::Start) => Ok(self.enter(S::Recording)),
            (S::Recording, I::Partial(text)) => Ok(vec![StateEvent::Partial(text)]),
            (S::Recording, I::Stop) => Ok(self.enter(S::Transcribing)),
            (S::Transcribing, I::Transcribed(text)) => {
                let mut events = self.enter(S::Polishing);
                events.push(StateEvent::Partial(text));
                Ok(events)
            }
            (S::Polishing, I::Polished(text)) => {
                let mut events = self.enter(S::Injecting);
                events.push(StateEvent::Final(text));
                Ok(events)
            }
            (S::Injecting, I::Injected) => Ok(self.enter(S::Idle)),
            (S::Error, I::Dismiss) => Ok(self.enter(S::Idle)),
            (_, other) => Err(self.illegal(Self::name_of(&other))),
        }
    }

    fn enter(&mut self, next: State) -> Vec<StateEvent> {
        self.state = next;
        vec![StateEvent::Entered(next)]
    }

    const fn illegal(&self, input: &'static str) -> IllegalTransition {
        IllegalTransition {
            state: self.state,
            input,
        }
    }

    const fn name_of(input: &Input) -> &'static str {
        match input {
            Input::Start => "Start",
            Input::Stop => "Stop",
            Input::Partial(_) => "Partial",
            Input::Transcribed(_) => "Transcribed",
            Input::Polished(_) => "Polished",
            Input::Injected => "Injected",
            Input::Fail(_) => "Fail",
            Input::Dismiss => "Dismiss",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(s: &str) -> String {
        s.to_owned()
    }

    #[test]
    fn happy_path_returns_to_idle() {
        let mut m = Machine::new();
        assert_eq!(
            m.apply(Input::Start).unwrap(),
            vec![StateEvent::Entered(State::Recording)]
        );
        assert_eq!(
            m.apply(Input::Partial(text("hello"))).unwrap(),
            vec![StateEvent::Partial(text("hello"))]
        );
        assert_eq!(
            m.apply(Input::Stop).unwrap(),
            vec![StateEvent::Entered(State::Transcribing)]
        );
        assert_eq!(
            m.apply(Input::Transcribed(text("hello there"))).unwrap(),
            vec![
                StateEvent::Entered(State::Polishing),
                StateEvent::Partial(text("hello there"))
            ]
        );
        assert_eq!(
            m.apply(Input::Polished(text("Hello there."))).unwrap(),
            vec![
                StateEvent::Entered(State::Injecting),
                StateEvent::Final(text("Hello there."))
            ]
        );
        assert_eq!(
            m.apply(Input::Injected).unwrap(),
            vec![StateEvent::Entered(State::Idle)]
        );
        assert_eq!(m.state(), State::Idle);
    }

    #[test]
    fn failure_is_reachable_from_every_running_state() {
        let paths: Vec<Vec<Input>> = vec![
            vec![Input::Start],
            vec![Input::Start, Input::Stop],
            vec![Input::Start, Input::Stop, Input::Transcribed(text("x"))],
            vec![
                Input::Start,
                Input::Stop,
                Input::Transcribed(text("x")),
                Input::Polished(text("X")),
            ],
        ];
        for path in paths {
            let mut m = Machine::new();
            for input in path {
                m.apply(input).unwrap();
            }
            let events = m.apply(Input::Fail(text("no microphone"))).unwrap();
            assert_eq!(m.state(), State::Error);
            assert_eq!(
                events,
                vec![
                    StateEvent::Entered(State::Error),
                    StateEvent::Failed(text("no microphone"))
                ]
            );
        }
    }

    #[test]
    fn idle_cannot_fail() {
        let mut m = Machine::new();
        assert!(m.apply(Input::Fail(text("boom"))).is_err());
        assert_eq!(m.state(), State::Idle);
    }

    #[test]
    fn illegal_transition_leaves_state_untouched() {
        let mut m = Machine::new();
        let err = m.apply(Input::Injected).unwrap_err();
        assert_eq!(err.state, State::Idle);
        assert_eq!(err.input, "Injected");
        assert_eq!(m.state(), State::Idle);
    }

    #[test]
    fn error_clears_only_on_dismiss() {
        let mut m = Machine::new();
        m.apply(Input::Start).unwrap();
        m.apply(Input::Fail(text("boom"))).unwrap();
        assert!(m.apply(Input::Start).is_err());
        assert_eq!(
            m.apply(Input::Dismiss).unwrap(),
            vec![StateEvent::Entered(State::Idle)]
        );
        assert_eq!(m.state(), State::Idle);
    }
}
