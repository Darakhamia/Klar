//! Polishing through the model Klar ships, running as a child process.
//!
//! The model itself lives in `klar-llm`, which is a binary rather than a
//! module. Its `lib.rs` carries the long version of why; the short one is that
//! whisper.cpp and llama.cpp each statically link their own copy of ggml, the
//! two copies are different versions, and a single binary containing both links
//! cleanly and then runs on whichever half the linker happened to pick. This
//! file is the parent's end of the pipe that avoids that.
//!
//! Three things follow from the child being a separate process, and they are
//! the design of everything below.
//!
//! **It can die, and that must be survivable.** A model that runs out of VRAM
//! kills the child. `CLAUDE.md` forbids a panic anywhere reachable from hotkey
//! handling, because a crash there kills a background app the user cannot see.
//! So every failure in here is a [`PolishError`], the pipeline's answer to one
//! is the answer it already has — insert the transcript unpolished — and
//! nothing in this file unwraps.
//!
//! **Its output has to be drained even when nobody is waiting.** A pipe holds a
//! few kilobytes; a child writing into a full one blocks. If Klar gave up on a
//! slow reply and simply stopped reading, the child would block writing it and
//! never reach the next request, and the polisher would be wedged rather than
//! merely late. Both of the child's output streams are therefore read by tasks
//! that run whether or not a dictation is in flight.
//!
//! **A late reply must not be read as the answer to the next request.** That is
//! what the id on the wire is for, and [`Link::reply_to`] is where it is spent.

use super::{PolishError, PolishRequest, Strength, TextPolisher, guard, system_prompt};
use klar_llm::protocol::{Request, Response};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct SidecarConfig {
    /// The `klar-llm` executable. Resolved by the caller — in a release build
    /// it sits beside Klar's own binary, in development it is whatever
    /// `cargo build` produced, and neither is this crate's business.
    pub program: PathBuf,
    /// The GGUF to load.
    pub model: PathBuf,
    /// How long the model gets before the transcript is used instead.
    pub budget: Duration,
    /// How long loading may take before the sidecar is declared broken.
    ///
    /// Generous, and it costs nothing to be: this is paid once, at launch, with
    /// nobody waiting on it. A cold read of a 2 GB file from a spinning disk is
    /// slower than anyone expects.
    pub load_budget: Duration,
    /// The most tokens any single reply may run to. A per-request cap is
    /// computed from the transcript as well — see [`Link::cap_for`] — and this
    /// is the ceiling on that.
    pub max_tokens: u32,
    /// Context window for the child. A dictation is a sentence or two.
    pub ctx: u32,
    /// Layers to offload. The child asks for more than any card has and lets
    /// llama.cpp keep on the CPU whatever does not fit.
    pub gpu_layers: u32,
}

impl Default for SidecarConfig {
    fn default() -> Self {
        Self {
            program: PathBuf::from("klar-llm"),
            model: PathBuf::new(),
            budget: Duration::from_millis(400),
            load_budget: Duration::from_secs(120),
            max_tokens: 512,
            ctx: 2048,
            gpu_layers: 999,
        }
    }
}

/// What the child reported about itself once the model was in memory.
#[derive(Debug, Clone)]
pub struct Ready {
    pub model: String,
    /// What llama.cpp says it is running on, so a machine quietly on the CPU
    /// can be told so in the settings window rather than just being slow.
    pub backend: String,
    pub load_ms: u64,
}

/// A running `klar-llm`, with its model already loaded.
///
/// Held for the life of the app. Loading is seconds and the budget is 400 ms,
/// so a polisher that started the model per dictation would miss the budget
/// every time by two orders of magnitude.
pub struct Sidecar {
    child: Child,
    link: Link<ChildStdin>,
    ready: Ready,
}

impl Sidecar {
    /// Start the child and wait for it to finish loading.
    ///
    /// Returns once the model is in memory, so the first dictation does not pay
    /// for it. Every error path here drops `child`, which kills it —
    /// see `kill_on_drop` below.
    pub async fn start(config: SidecarConfig) -> Result<Self, PolishError> {
        let mut command = Command::new(&config.program);
        command
            .arg("--model")
            .arg(&config.model)
            .arg("--ctx")
            .arg(config.ctx.to_string())
            .arg("--gpu-layers")
            .arg(config.gpu_layers.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // Klar can be quit, killed, or crash. None of those may leave a
            // process behind holding a model in VRAM, and a user who has to
            // find one in Task Manager has been failed twice.
            .kill_on_drop(true);

        // Without this, every launch of the child flashes a console window over
        // whatever the user is doing — Klar has no console of its own, so
        // Windows creates one for the child.
        #[cfg(windows)]
        {
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = command.spawn().map_err(|error| {
            PolishError::Unstartable(format!("{}: {error}", config.program.display()))
        })?;

        let (Some(to_child), Some(from_child)) = (child.stdin.take(), child.stdout.take()) else {
            return Err(PolishError::Unstartable(
                "the child was started without the pipes it was asked for".to_owned(),
            ));
        };
        if let Some(stderr) = child.stderr.take() {
            fold_into_log(stderr);
        }

        // Small: the protocol is one request at a time, and the only thing that
        // can queue up behind it is a reply nobody wants any more.
        let (replies_out, replies) = mpsc::channel(8);
        pump(from_child, replies_out);

        let mut link = Link {
            to_child,
            replies,
            next_id: 0,
            budget: config.budget,
            max_tokens: config.max_tokens,
        };

        let ready = match tokio::time::timeout(config.load_budget, link.replies.recv()).await {
            Err(_) => {
                return Err(PolishError::Unstartable(format!(
                    "{} did not finish loading within {:?}",
                    config.model.display(),
                    config.load_budget
                )));
            }
            Ok(None) => {
                return Err(PolishError::Unstartable(
                    "it exited before the model was ready — its output is in the log".to_owned(),
                ));
            }
            Ok(Some(Err(error))) => return Err(PolishError::Unreadable(error)),
            Ok(Some(Ok(Response::Ready {
                model,
                backend,
                load_ms,
            }))) => Ready {
                model,
                backend,
                load_ms,
            },
            Ok(Some(Ok(other))) => {
                return Err(PolishError::Unreadable(format!(
                    "expected the ready line first, got {other:?}"
                )));
            }
        };

        tracing::info!(
            model = %ready.model,
            backend = %ready.backend,
            load_ms = ready.load_ms,
            "the polish model is loaded"
        );

        Ok(Self { child, link, ready })
    }

    /// What the child reported at startup, for the settings window to show.
    pub fn ready(&self) -> &Ready {
        &self.ready
    }

    /// The model's answer with [`guard`] not applied, and what it cost.
    ///
    /// Not for the pipeline, which must never insert unguarded text. This is
    /// for `klar-cli polish`, where the question being asked is what the model
    /// actually said: a rejection reports that the result was 380% of the
    /// transcript, and the only way to know whether that is a model writing an
    /// essay or a prompt that needs a sentence changed is to read it.
    pub async fn polish_unguarded(
        &mut self,
        request: PolishRequest<'_>,
    ) -> Result<Polished, PolishError> {
        self.link.ask(request).await
    }
}

/// One answer from the model, before anything has judged it.
#[derive(Debug, Clone)]
pub struct Polished {
    pub text: String,
    /// The child's own measurement, which excludes the pipe and the parent's
    /// scheduling. Lower than what the user waits for, and the right number for
    /// comparing two models.
    pub elapsed_ms: u64,
    pub tokens: u32,
}

/// The sidecar as it sits beside the running binary.
///
/// Where both callers find it: Tauri's `externalBin` places it next to Klar's
/// own executable in an installed build, and `cargo build` puts it next to
/// `klar-cli` in a development one. `None` if the current executable cannot be
/// located, which is not a case worth a distinct error — it means the same
/// thing as not finding the file.
pub fn beside_current_exe() -> Option<PathBuf> {
    let name = if cfg!(windows) {
        "klar-llm.exe"
    } else {
        "klar-llm"
    };
    let beside = std::env::current_exe().ok()?.with_file_name(name);
    beside.is_file().then_some(beside)
}

impl TextPolisher for Sidecar {
    async fn polish(&mut self, request: PolishRequest<'_>) -> Result<String, PolishError> {
        self.link.polish(request).await
    }

    async fn available(&mut self) -> bool {
        // `Ok(None)` is tokio's "still running". An error from `try_wait` means
        // the child cannot be asked about, which is not a working polisher
        // either.
        matches!(self.child.try_wait(), Ok(None))
    }
}

/// The conversation, with no process attached.
///
/// Split out from [`Sidecar`] so the part that can be wrong — framing, the
/// budget, discarding a late reply — can be tested over a pair of pipes in
/// memory. A test that had to build llama.cpp and load a model to check that a
/// stale id is ignored would not be written, and then that would not be checked.
struct Link<W> {
    to_child: W,
    replies: mpsc::Receiver<Result<Response, String>>,
    next_id: u64,
    budget: Duration,
    max_tokens: u32,
}

impl<W: AsyncWrite + Unpin + Send> Link<W> {
    async fn polish(&mut self, request: PolishRequest<'_>) -> Result<String, PolishError> {
        if request.strength.prompt().is_none() {
            // Verbatim reaching here is a caller bug, not a reason to hand the
            // user's words to a model.
            return Ok(request.text.to_owned());
        }

        let strength = request.strength;
        let text = request.text;
        let answer = self.ask(request).await?;
        Ok(guard(text, &answer.text, strength)?)
    }

    /// Ask, and hand back whatever came out. Judging it is the caller's job —
    /// which for everything but `klar-cli` means [`Link::polish`].
    async fn ask(&mut self, request: PolishRequest<'_>) -> Result<Polished, PolishError> {
        let Some(instructions) = request.strength.prompt() else {
            return Ok(Polished {
                text: request.text.to_owned(),
                elapsed_ms: 0,
                tokens: 0,
            });
        };

        self.next_id += 1;
        let id = self.next_id;

        self.send(&Request {
            id,
            system: system_prompt(instructions, &request),
            user: request.text.to_owned(),
            max_tokens: self.cap_for(request.text, request.strength),
        })
        .await?;

        match self.reply_to(id).await? {
            Response::Done {
                text,
                elapsed_ms,
                tokens,
                ..
            } => {
                tracing::debug!(elapsed_ms, tokens, "polished");
                Ok(Polished {
                    text,
                    elapsed_ms,
                    tokens,
                })
            }
            Response::Failed { message, .. } => Err(PolishError::Model(message)),
            Response::Ready { .. } => Err(PolishError::Unreadable(
                "the model announced itself twice".to_owned(),
            )),
        }
    }

    async fn send(&mut self, request: &Request) -> Result<(), PolishError> {
        let mut line = serde_json::to_string(request)
            .map_err(|error| PolishError::Unreadable(error.to_string()))?;
        line.push('\n');

        // A broken pipe here means the child is gone. It is the same condition
        // the reader will report, but it surfaces on this side first.
        self.to_child
            .write_all(line.as_bytes())
            .await
            .map_err(|error| PolishError::Crashed(error.to_string()))?;
        self.to_child
            .flush()
            .await
            .map_err(|error| PolishError::Crashed(error.to_string()))
    }

    /// Wait for the answer to `want`, within the budget.
    ///
    /// Anything else that arrives is discarded rather than returned. The case
    /// that matters: a dictation whose polish ran over the budget was abandoned
    /// and inserted unpolished, but the child did not know that and finished
    /// generating anyway. Its answer is still in the pipe when the next
    /// dictation asks its question, and without the id it would be handed back
    /// as the reply — the previous sentence, pasted into the current one.
    ///
    /// A stale reply still costs the request behind it whatever was left of the
    /// child's generation, because the child is single-threaded and had not
    /// reached the new request yet. That is honest: it was busy. The budget
    /// expires and the transcript is used, which is what happens whenever the
    /// model is too slow, for whatever reason.
    async fn reply_to(&mut self, want: u64) -> Result<Response, PolishError> {
        let deadline = tokio::time::Instant::now() + self.budget;

        loop {
            match tokio::time::timeout_at(deadline, self.replies.recv()).await {
                Err(_) => return Err(PolishError::TooSlow(self.budget)),
                Ok(None) => {
                    return Err(PolishError::Crashed(
                        "it closed its output — its last words are in the log".to_owned(),
                    ));
                }
                Ok(Some(Err(error))) => return Err(PolishError::Unreadable(error)),
                Ok(Some(Ok(reply))) => match reply {
                    Response::Done { id, .. } | Response::Failed { id, .. } if id != want => {
                        tracing::debug!(id, want, "discarded a reply that was given up on");
                    }
                    Response::Ready { .. } => {
                        tracing::warn!("the model announced itself again mid-conversation");
                    }
                    wanted => return Ok(wanted),
                },
            }
        }
    }

    /// A ceiling on the reply, in tokens, taken from the guard that will judge
    /// it.
    ///
    /// [`guard`] throws away anything longer than the strength's ceiling, so
    /// every token generated past that point is latency spent on text that
    /// cannot be used — and the failure that produces it, a model that has
    /// started writing something of its own, is exactly the one that runs long.
    /// Stopping near the ceiling turns a rejection that took a second into one
    /// that took the budget.
    ///
    /// Two characters per token rather than the four that English prose
    /// averages, because Cyrillic and CJK are closer to one and a cap that is
    /// too tight would truncate a legitimate cleanup into a rejection.
    fn cap_for(&self, text: &str, strength: Strength) -> u32 {
        let chars = u32::try_from(text.chars().count()).unwrap_or(u32::MAX);
        let ceiling = strength.bounds().1;
        let allowed = (f64::from(chars) * f64::from(ceiling) / 2.0) as u32;

        // The floor keeps a two-word dictation from being capped below the
        // punctuation it came for.
        allowed.saturating_add(24).clamp(64, self.max_tokens)
    }
}

/// Read the child's replies for as long as it has any, whether or not a
/// dictation is waiting for one.
///
/// Parsing happens here so a malformed line is reported as one bad reply rather
/// than desynchronising the stream — the child writes one JSON object per line,
/// so the line after a broken one is still a whole message.
fn pump<R>(from_child: R, replies: mpsc::Sender<Result<Response, String>>)
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut lines = BufReader::new(from_child).lines();
        loop {
            match lines.next_line().await {
                Ok(Some(line)) if line.trim().is_empty() => {}
                Ok(Some(line)) => {
                    let parsed = serde_json::from_str::<Response>(&line).map_err(|error| {
                        format!("{error}: {}", line.chars().take(200).collect::<String>())
                    });
                    if replies.send(parsed).await.is_err() {
                        // Nobody is listening any more, which means the Sidecar
                        // was dropped and the child is being killed.
                        break;
                    }
                }
                Ok(None) => break,
                Err(error) => {
                    let _ = replies.send(Err(error.to_string())).await;
                    break;
                }
            }
        }
    });
}

/// Fold the child's stderr into Klar's own log.
///
/// llama.cpp is loud, and all of it goes to stderr because stdout carries the
/// protocol. This is not only for the log: an undrained pipe fills, and a child
/// blocked writing its startup banner never gets to the model.
fn fold_into_log<R>(stderr: R)
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            tracing::debug!(target: "klar_llm", "{line}");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{DuplexStream, WriteHalf};

    /// A link with a pipe where the child would be, and the other end of that
    /// pipe for the test to play the child on.
    fn linked(budget: Duration) -> (Link<WriteHalf<DuplexStream>>, DuplexStream) {
        let (ours, theirs) = tokio::io::duplex(4096);
        let (from_child, to_child) = tokio::io::split(ours);

        let (replies_out, replies) = mpsc::channel(8);
        pump(from_child, replies_out);

        let link = Link {
            to_child,
            replies,
            next_id: 0,
            budget,
            max_tokens: 512,
        };
        (link, theirs)
    }

    async fn read_request(child: &mut DuplexStream) -> Request {
        let mut line = String::new();
        BufReader::new(child)
            .read_line(&mut line)
            .await
            .expect("the parent wrote a request");
        serde_json::from_str(&line).expect("and it was a request")
    }

    async fn write_reply(child: &mut DuplexStream, reply: &Response) {
        let mut line = serde_json::to_string(reply).expect("serialises");
        line.push('\n');
        child.write_all(line.as_bytes()).await.expect("writes");
    }

    /// The invariant the id exists for.
    ///
    /// A polish that ran over the budget is abandoned, but the child keeps
    /// generating and its answer arrives during the *next* dictation. Without
    /// the id that answer would be returned as the new one, and the previous
    /// sentence would be pasted into the current one — silently, and only
    /// sometimes, which is the worst way for it to be wrong.
    #[tokio::test]
    async fn a_late_reply_is_never_the_answer_to_the_next_request() {
        let (mut link, mut child) = linked(Duration::from_millis(80));

        let first = "so um I think we should ship it on Friday";
        let abandoned = link
            .polish(PolishRequest {
                text: first,
                strength: Strength::Balanced,
                vocabulary: &[],
                language: None,
            })
            .await
            .expect_err("the child says nothing in time");
        assert!(
            matches!(abandoned, PolishError::TooSlow(_)),
            "expected the budget to expire, got {abandoned}"
        );

        // The child, which never knew it had been given up on, now answers.
        let asked = read_request(&mut child).await;
        assert_eq!(asked.id, 1);
        write_reply(
            &mut child,
            &Response::Done {
                id: 1,
                text: "I think we should ship it on Friday.".to_owned(),
                elapsed_ms: 900,
                tokens: 12,
            },
        )
        .await;

        // And the next dictation goes out while that reply is in flight.
        let second = "the invoice needs to go out before the end of the month";
        let polish = tokio::spawn(async move {
            let asked = read_request(&mut child).await;
            assert_eq!(asked.id, 2);
            write_reply(
                &mut child,
                &Response::Done {
                    id: 2,
                    text: "The invoice needs to go out before the end of the month.".to_owned(),
                    elapsed_ms: 40,
                    tokens: 14,
                },
            )
            .await;
            // Held open: dropping the child end mid-test would look like a crash.
            tokio::time::sleep(Duration::from_millis(200)).await;
        });

        let polished = link
            .polish(PolishRequest {
                text: second,
                strength: Strength::Balanced,
                vocabulary: &[],
                language: None,
            })
            .await
            .expect("the second dictation is answered");

        assert_eq!(
            polished, "The invoice needs to go out before the end of the month.",
            "the answer to the abandoned request was returned as this one's"
        );
        polish.abort();
    }

    /// A child that dies has to surface as an error. The alternative is a
    /// polisher that waits out its budget on every dictation for the rest of
    /// the session.
    #[tokio::test]
    async fn a_child_that_dies_is_an_error_and_not_a_hang() {
        let (mut link, child) = linked(Duration::from_secs(30));
        drop(child);

        let started = std::time::Instant::now();
        let error = link
            .polish(PolishRequest {
                text: "anything at all",
                strength: Strength::Light,
                vocabulary: &[],
                language: None,
            })
            .await
            .expect_err("the child is gone");

        assert!(
            matches!(error, PolishError::Crashed(_)),
            "expected a crash, got {error}"
        );
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "it waited out the budget instead of noticing the child had gone"
        );
    }

    /// Verbatim is the promise that the text never reaches a model. The pipe is
    /// dead on purpose: anything that tried to use it would fail here.
    #[tokio::test]
    async fn verbatim_never_reaches_the_model() {
        let (mut link, child) = linked(Duration::from_millis(400));
        drop(child);

        let text = "left exactly as it was, um, including this";
        let polished = link
            .polish(PolishRequest {
                text,
                strength: Strength::Verbatim,
                vocabulary: &[],
                language: None,
            })
            .await
            .expect("verbatim cannot fail");

        assert_eq!(polished, text);
    }

    /// The model's own failure is reported as the model's, not as a broken pipe
    /// or a bad reply. It is the one error whose message came from the machine
    /// the user is sitting at.
    #[tokio::test]
    async fn a_failure_from_the_model_keeps_its_message() {
        let (mut link, mut child) = linked(Duration::from_secs(5));

        let answering = tokio::spawn(async move {
            let asked = read_request(&mut child).await;
            write_reply(
                &mut child,
                &Response::Failed {
                    id: asked.id,
                    message: "the context window is smaller than the prompt".to_owned(),
                },
            )
            .await;
            tokio::time::sleep(Duration::from_millis(200)).await;
        });

        let error = link
            .polish(PolishRequest {
                text: "a long dictation",
                strength: Strength::Balanced,
                vocabulary: &[],
                language: None,
            })
            .await
            .expect_err("the model refused");

        match error {
            PolishError::Model(message) => {
                assert!(message.contains("context window"), "{message}");
            }
            other => panic!("expected the model's own failure, got {other}"),
        }
        answering.abort();
    }

    /// The words the user taught Klar have to reach the model, or the
    /// dictionary silently stops applying to this stage.
    #[tokio::test]
    async fn the_dictionary_is_part_of_what_the_model_is_told() {
        let (mut link, mut child) = linked(Duration::from_secs(5));

        let vocabulary = vec!["Klar".to_owned(), "Darakhamia".to_owned()];
        let asking = tokio::spawn(async move {
            let asked = read_request(&mut child).await;
            assert!(
                asked.system.contains("Klar") && asked.system.contains("Darakhamia"),
                "the dictionary did not reach the model: {}",
                asked.system
            );
            tokio::time::sleep(Duration::from_millis(200)).await;
        });

        let _ = link
            .polish(PolishRequest {
                text: "send it to Darakhamia",
                strength: Strength::Light,
                vocabulary: &vocabulary,
                language: None,
            })
            .await;

        asking.await.expect("the child end checked the prompt");
    }

    /// The cap has to leave room for a real cleanup and still be a cap. A
    /// version that returned the floor for everything would truncate long
    /// dictations; one that returned the ceiling would not be doing anything.
    #[test]
    fn the_token_cap_grows_with_the_dictation_and_still_has_a_ceiling() {
        let (link, _child) = {
            let (ours, theirs) = tokio::io::duplex(64);
            let (_, to_child) = tokio::io::split(ours);
            let (_out, replies) = mpsc::channel(1);
            (
                Link {
                    to_child,
                    replies,
                    next_id: 0,
                    budget: Duration::from_millis(400),
                    max_tokens: 512,
                },
                theirs,
            )
        };

        let short = link.cap_for("yes", Strength::Balanced);
        let sentence = link.cap_for(&"word ".repeat(40), Strength::Balanced);
        let essay = link.cap_for(&"word ".repeat(2000), Strength::Balanced);

        assert_eq!(short, 64, "a two-word dictation gets the floor");
        assert!(
            sentence > short && sentence < 512,
            "a sentence should sit between the floor and the ceiling, got {sentence}"
        );
        assert_eq!(essay, 512, "and nothing may exceed the configured ceiling");
    }

    /// Starting a program that is not there is a bad install or a bad setting,
    /// and it has to arrive as an error rather than as a panic in a background
    /// thread nobody sees.
    #[tokio::test]
    async fn a_missing_binary_is_reported_and_not_a_panic() {
        // Matched rather than `expect_err`: a Sidecar owns a running child
        // and is deliberately not Debug, because printing one would mean
        // printing its pipes.
        let started = Sidecar::start(SidecarConfig {
            program: PathBuf::from("klar-llm-that-was-never-installed"),
            model: PathBuf::from("nothing.gguf"),
            load_budget: Duration::from_secs(2),
            ..SidecarConfig::default()
        })
        .await;
        let Err(error) = started else {
            panic!("there is no such program, so it cannot have started");
        };

        assert!(
            matches!(error, PolishError::Unstartable(_)),
            "expected unstartable, got {error}"
        );
    }
}
