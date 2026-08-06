//! The sidecar: load a model, answer one polish request at a time, say nothing
//! else on stdout.
//!
//! See `lib.rs` for why this is a separate process rather than a module.
//!
//! Two rules hold everywhere below. **stdout carries the protocol and nothing
//! else** — llama.cpp is loud, and one stray line of its logging on stdout
//! makes the parent's next `serde_json` call fail on a message it cannot even
//! quote back; everything human-readable goes to stderr, which Klar folds into
//! its own log. And **the process is disposable**: it holds no state the parent
//! cannot rebuild, so anything that goes wrong may end with the process
//! exiting. The parent notices, inserts the transcript unpolished, and starts
//! another one.

use anyhow::{Context, Result, bail};
use clap::Parser;
use klar_llm::protocol::{Request, Response};
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaChatMessage, LlamaModel};
use llama_cpp_2::sampling::LlamaSampler;
use std::io::{BufRead, Write};
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Parser)]
#[command(about = "Klar's polish model, speaking line-delimited JSON on stdin and stdout.")]
struct Args {
    /// The GGUF to load.
    #[arg(long)]
    model: PathBuf,

    /// Context window. A dictation is a sentence or two and the prompt is
    /// short; the default is generous for both and keeps the KV cache small,
    /// which is most of what the model costs in memory when idle.
    #[arg(long, default_value_t = 2048)]
    ctx: u32,

    /// How many layers to put on the GPU. The default offloads everything it
    /// can: on a machine with no usable device llama.cpp keeps them on the CPU
    /// by itself, so asking for more than exists is not an error.
    #[arg(long, default_value_t = 999)]
    gpu_layers: u32,

    /// CPU threads for whatever is not offloaded. Zero lets llama.cpp choose.
    #[arg(long, default_value_t = 0)]
    threads: i32,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Before anything is loaded: llama.cpp writes to stdout unless told
    // otherwise, and stdout is the protocol.
    llama_cpp_2::send_logs_to_tracing(llama_cpp_2::LogOptions::default().with_logs_enabled(false));

    let started = Instant::now();
    let backend = LlamaBackend::init().context("llama backend")?;

    let model_params = LlamaModelParams::default().with_n_gpu_layers(args.gpu_layers);
    let model = LlamaModel::load_from_file(&backend, &args.model, &model_params)
        .with_context(|| format!("loading {}", args.model.display()))?;

    let ctx = NonZeroU32::new(args.ctx.max(512)).unwrap_or(NonZeroU32::MIN);
    let mut context_params = LlamaContextParams::default().with_n_ctx(Some(ctx));
    if args.threads > 0 {
        context_params = context_params.with_n_threads(args.threads);
    }
    let mut context = model
        .new_context(&backend, context_params)
        .context("creating the context")?;

    // The template baked into the GGUF, when it has one. A model prompted
    // without its own template answers the instruction instead of following
    // it, which is the exact failure `klar_core::polish::guard` exists to
    // catch — better not to cause it here.
    let template = model.chat_template(None).ok();

    let name = args.model.file_stem().map_or_else(
        || args.model.display().to_string(),
        |s| s.to_string_lossy().into_owned(),
    );

    say(&Response::Ready {
        model: name,
        backend: describe_backend(&model, args.gpu_layers),
        load_ms: elapsed_ms(started),
    })?;

    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let line = line.context("reading a request")?;
        if line.trim().is_empty() {
            continue;
        }

        let request: Request = match serde_json::from_str(&line) {
            Ok(request) => request,
            Err(error) => {
                // No id to answer with, so this cannot be reported down the
                // protocol. The parent will time the request out.
                eprintln!("klar-llm: unreadable request: {error}");
                continue;
            }
        };

        let id = request.id;
        let started = Instant::now();
        match generate(&model, &mut context, template.as_ref(), &request) {
            Ok((text, tokens)) => say(&Response::Done {
                id,
                text,
                elapsed_ms: elapsed_ms(started),
                tokens,
            })?,
            Err(error) => say(&Response::Failed {
                id,
                message: format!("{error:#}"),
            })?,
        }
    }

    Ok(())
}

/// One request, start to finish.
///
/// The KV cache is cleared first rather than reused across requests. Each
/// dictation stands alone, exactly as it does in the ASR stage, and carrying
/// context between them is how a model starts tidying the previous sentence
/// into this one.
fn generate(
    model: &LlamaModel,
    context: &mut llama_cpp_2::context::LlamaContext,
    template: Option<&llama_cpp_2::model::LlamaChatTemplate>,
    request: &Request,
) -> Result<(String, u32)> {
    context.clear_kv_cache();

    let prompt = match template {
        Some(template) => {
            let messages = vec![
                LlamaChatMessage::new("system".to_owned(), request.system.clone())?,
                LlamaChatMessage::new("user".to_owned(), request.user.clone())?,
            ];
            model.apply_chat_template(template, &messages, true)?
        }
        // No template in the GGUF. ChatML is the most common shape and the one
        // most instruct models were tuned near; it is a guess, and it is said
        // out loud on stderr rather than pretended about.
        None => {
            eprintln!("klar-llm: the model carries no chat template; assuming ChatML");
            format!(
                "<|im_start|>system\n{}<|im_end|>\n<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n",
                request.system, request.user
            )
        }
    };

    let tokens = model
        .str_to_token(&prompt, AddBos::Always)
        .context("tokenising the prompt")?;

    let room = context.n_ctx() as usize;
    if tokens.len() + request.max_tokens as usize >= room {
        bail!(
            "the prompt is {} tokens and the reply may be {}, which does not fit a {room}-token context",
            tokens.len(),
            request.max_tokens
        );
    }

    let prompt_length = tokens.len();
    let mut batch = LlamaBatch::new(room, 1);
    for (position, token) in tokens.into_iter().enumerate() {
        // Logits only for the final token: the ones before it are prompt, and
        // asking for their distributions is work whose answer is discarded.
        batch.add(token, position as i32, &[0], position == prompt_length - 1)?;
    }
    context.decode(&mut batch).context("decoding the prompt")?;

    // Nearly greedy. This stage removes fillers and fixes punctuation; there is
    // one right answer and no reason to sample away from it. The small
    // temperature is there because pure greedy decoding is the setting where a
    // model that starts repeating a phrase never stops.
    let mut sampler = LlamaSampler::chain_simple([
        LlamaSampler::penalties(64, 1.1, 0.0, 0.0),
        LlamaSampler::temp(0.2),
        LlamaSampler::top_p(0.9, 1),
        LlamaSampler::dist(seed()),
    ]);

    let mut produced = 0_u32;
    // Bytes, not a string. A token can be half a multi-byte character — a
    // single em dash arrives as two of them in most vocabularies — and
    // decoding each token on its own turns it into two replacement characters.
    // Accumulate raw and decode once, at the end.
    let mut out: Vec<u8> = Vec::new();

    // The absolute position of the next token in the sequence, which is where
    // the prompt left off. Not derivable from the batch: after the first
    // generated token the batch holds one entry and its index is zero.
    let mut position = prompt_length as i32;
    let mut cursor = batch.n_tokens() - 1;

    while produced < request.max_tokens {
        let token = sampler.sample(context, cursor);
        if model.is_eog_token(token) {
            break;
        }

        match model.token_to_piece_bytes(token, 32, false, None) {
            Ok(bytes) => out.extend_from_slice(&bytes),
            // One unreadable token is not worth abandoning the reply over; the
            // guard in klar-core judges the whole result anyway.
            Err(error) => eprintln!("klar-llm: token {token:?} would not decode: {error}"),
        }
        produced += 1;

        batch.clear();
        batch.add(token, position, &[0], true)?;
        position += 1;
        cursor = 0;
        context.decode(&mut batch).context("decoding a token")?;
    }

    Ok((String::from_utf8_lossy(&out).trim().to_owned(), produced))
}

/// A different seed per process, so a model that latches onto a bad phrasing
/// does not do it identically for the rest of the session.
fn seed() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.subsec_nanos())
}

/// What is doing the work, in the same spirit as `klar_core::asr::devices`: a
/// machine that quietly fell back to the CPU is not broken, it is ten times
/// slower, and nothing else would say so.
fn describe_backend(model: &LlamaModel, requested: u32) -> String {
    if requested == 0 {
        return "cpu (no layers offloaded)".to_owned();
    }
    format!(
        "{} layers requested, {} in the model",
        requested,
        model.n_layer()
    )
}

fn elapsed_ms(since: Instant) -> u64 {
    u64::try_from(since.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// One response, one line, flushed.
///
/// Without the flush the parent waits on a reply sitting in this process's
/// buffer, and the symptom is every polish timing out at exactly the budget.
fn say(response: &Response) -> Result<()> {
    let line = serde_json::to_string(response)?;
    let mut stdout = std::io::stdout().lock();
    writeln!(stdout, "{line}")?;
    stdout.flush()?;
    Ok(())
}
