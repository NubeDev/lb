//! The **child-side serve loop** — the reactor every native (Tier-2) extension runs
//! (native-call-concurrency scope). This is the counterpart of `conn.rs`: that multiplexes the
//! HOST's end of the line, this one keeps the CHILD's end from re-serializing it.
//!
//! It lives here, in the crate that already defines the wire types both ends share, so the loop is
//! written **once** and every native extension inherits it. Copy-pasting a reactor per extension is
//! how the two ends drift, and a child that awaits each handler silently caps the whole transport at
//! concurrency 1 no matter what the host does.
//!
//! **This is why the host-side fix alone measures nothing.** The old federation loop read a frame,
//! `.await`ed the handler to completion, wrote the reply, and only then read the next frame — its
//! `tokio::spawn` was a panic fence that was immediately joined, not concurrency. Removing the host
//! mutex without this would just move the queue from the mutex into the pipe.
//!
//! Three properties this file exists to guarantee:
//!
//! 1. **Read and dispatch, never await the handler in the read path.** Each `call` is spawned; the
//!    loop goes straight back to reading. The spawn keeps its original job as a **panic fence** (a
//!    panic deep in a connector unwinds that task only) — now a real fence, since it is not joined.
//! 2. **Exactly one task owns stdout.** Handlers send replies over an mpsc to a single writer task.
//!    Two concurrent writers would interleave `Content-Length` headers with bodies and desynchronize
//!    the stream *permanently* — an unrecoverable failure that presents as random decode errors.
//! 3. **In-flight work is explicitly bounded.** A semaphore, not unbounded spawning, so a stampede
//!    queues instead of opening N simultaneous database connections.

use std::future::Future;
use std::sync::Arc;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{mpsc, Semaphore};

use crate::frame::{read_frame, write_frame};
use crate::rpc::{Method, Reply, Request};

/// How many `call` handlers may run at once in one child.
///
/// **Deliberately NOT derived from the pool cache's `MAX_ENTRIES = 16`.** That cap counts *distinct
/// warm sources*; this counts *concurrent calls*. Thirteen queries against one source is thirteen
/// here and **one** there — the two numbers measure different things and coupling them would be a
/// coincidence, not a design.
///
/// 8 is sized from what one source's connection can absorb: the observed serial staircase was ~0.93 s
/// per warm remote query, so 8 in flight covers a 13-tile dashboard in two waves while leaving the
/// remote database far from the ~33k-point workload it also serves. Past the bound calls queue (they
/// still complete) rather than fanning out unboundedly. A per-extension manifest field is the obvious
/// next step if one extension ever needs a different number; one constant beats a knob nobody sets.
pub const DEFAULT_MAX_IN_FLIGHT: usize = 8;

/// Run the child's control loop over `input`/`output` until the host closes the line or sends
/// `shutdown`.
///
/// `handle` is the extension's tool dispatcher: it receives one `call` [`Request`] and returns the
/// [`Reply`] to send back. It is invoked **concurrently** — see [`serve_with`] for the contract this
/// places on an extension's handlers.
pub async fn serve<R, W, F, Fut>(input: R, output: W, ext_id: String, handle: F)
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin + Send + 'static,
    F: Fn(Request) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Reply> + Send + 'static,
{
    serve_with(input, output, ext_id, DEFAULT_MAX_IN_FLIGHT, handle).await
}

/// [`serve`] with an explicit in-flight bound.
///
/// **Handlers must be concurrency-safe.** Before this loop they were implicitly serialized by the
/// transport — only one ran at a time, so a handler could mutate process-global state without
/// synchronization and never notice. That accidental mutual exclusion is gone: `handle` is now
/// called from many tasks at once. An extension that genuinely needs serial execution must take its
/// own lock (or pass `max_in_flight = 1`). This is the breaking half of the SDK contract change.
pub async fn serve_with<R, W, F, Fut>(
    mut input: R,
    output: W,
    ext_id: String,
    max_in_flight: usize,
    handle: F,
) where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin + Send + 'static,
    F: Fn(Request) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Reply> + Send + 'static,
{
    // Property 2: ONE task owns the write half, forever. Everything else sends it replies.
    let (tx, mut rx) = mpsc::channel::<Reply>(max_in_flight * 2 + 8);
    let writer = tokio::spawn(async move {
        let mut output = output;
        while let Some(reply) = rx.recv().await {
            let Ok(bytes) = serde_json::to_vec(&reply) else {
                continue;
            };
            if write_frame(&mut output, &bytes).await.is_err() {
                break; // host closed the line
            }
        }
    });

    let limit = Arc::new(Semaphore::new(max_in_flight));
    let handle = Arc::new(handle);

    loop {
        let body = match read_frame(&mut input).await {
            Ok(b) => b,
            Err(_) => break, // host closed the line — exit cleanly
        };
        let req: Request = match serde_json::from_slice(&body) {
            Ok(r) => r,
            Err(_) => continue,
        };

        match req.method {
            // Answered inline: they are trivial and must not queue behind saturated tool calls.
            // A `health` poll that waits on 8 slow queries would report a healthy child as dead and
            // trip the restart policy under exactly the load this scope exists to support.
            Method::Init => {
                let _ = tx
                    .send(Reply::ok(
                        req.id,
                        format!(r#"{{"ready":true,"ext":"{ext_id}"}}"#),
                    ))
                    .await;
            }
            Method::Health => {
                let _ = tx.send(Reply::ok(req.id, "ok")).await;
            }
            Method::Shutdown => {
                let _ = tx.send(Reply::ok(req.id, "bye")).await;
                break;
            }
            // Property 1 + 3: bound, then spawn WITHOUT awaiting, and keep reading.
            Method::Call => {
                let Ok(permit) = Arc::clone(&limit).acquire_owned().await else {
                    break; // semaphore closed — shutting down
                };
                let handle = Arc::clone(&handle);
                let tx = tx.clone();
                let id = req.id;
                tokio::spawn(async move {
                    // The panic fence, now load-bearing: this task is never joined, so a panic in a
                    // connector unwinds only here. `JoinHandle` is dropped, so we convert a panic
                    // into an error reply by catching it at the task boundary below.
                    let reply = match tokio::spawn(async move { handle(req).await }).await {
                        Ok(reply) => reply,
                        Err(e) => Reply::err(id, format!("tool call panicked: {e}")),
                    };
                    let _ = tx.send(reply).await;
                    drop(permit);
                });
            }
        }
    }

    // Let the writer drain what is already queued, then stop it.
    drop(tx);
    let _ = writer.await;
}
