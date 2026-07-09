//! The **event hub** — the connection-scoped, ephemeral subscription registry behind the one
//! multiplexed SSE connection per browser session (unified-event-stream scope §3). No SurrealDB state
//! (scope "Data: none"): a dropped connection drops its subscriptions; a reconnect re-subscribes.
//!
//! Lifecycle:
//!   - `open()` mints a stream id (`sid`), registers a `Conn` holding an mpsc sender to the SSE task,
//!     and returns the receiver the `GET /events/stream` body folds into `event: mux` frames.
//!   - `subscribe(sid, subject, principal)` runs the subject's host gate ([`open_subject`]) and, on
//!     success, spawns ONE task that drives the subject's frame stream into the connection's sender
//!     (each frame wrapped `{sub, event, data}`). A gate failure pushes one opaque `error` mux frame and
//!     the connection lives on (scope: "deny is a per-subscription error frame, never a connection kill").
//!   - `unsubscribe(sid, subject)` aborts that subject's task; further server-side events for it emit
//!     nothing (scope "Unsubscribe").
//!   - `close(sid)` (the SSE body's drop guard) removes the connection and aborts every subject task —
//!     releasing the bus subscriptions the tasks held.
//!
//! Head-of-line fairness (scope "Risks"): the per-connection mpsc is BOUNDED; a subject task that would
//! block on a full channel drops the oldest by using `try_send` and, when full, evicting — motion is
//! fire-and-forget (rule 3), so a chatty subject cannot wedge the pipe. Run/flow catch-up is bounded by
//! the snapshot the host read, as today.

use std::collections::HashMap;
use std::sync::Arc;

use futures::stream::StreamExt;
use lb_auth::Principal;
use serde_json::json;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use super::subject::{open_subject, SubjectError};
use crate::state::Gateway;

/// A serialized mux frame ready to write to the SSE body: `data:` is the `{sub, event, data}` envelope.
pub type MuxLine = String;

/// The bounded depth of one connection's outbound queue. Generous enough that the dock's run feed never
/// visibly lags, bounded so a fast series can't grow it without limit (drop-oldest under backpressure).
const CONN_QUEUE_DEPTH: usize = 1024;

/// One live browser connection: the sender the subject tasks push mux lines to, plus the set of active
/// subject tasks (keyed by subject string, so a re-subscribe replaces and an unsubscribe aborts exactly one).
struct Conn {
    tx: mpsc::Sender<MuxLine>,
    subjects: HashMap<String, JoinHandle<()>>,
}

/// The process-wide hub: `sid -> Conn`. Held behind an `Arc` in [`Gateway`] and cloned per request.
#[derive(Clone, Default)]
pub struct EventHub {
    conns: Arc<Mutex<HashMap<String, Conn>>>,
}

impl EventHub {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new connection. Returns its minted `sid` and the receiver the SSE body folds. The
    /// caller emits `event: hello {sid}` first, then every mux line from this receiver.
    pub async fn open(&self) -> (String, mpsc::Receiver<MuxLine>) {
        let sid = lb_host::new_ulid();
        let (tx, rx) = mpsc::channel(CONN_QUEUE_DEPTH);
        self.conns.lock().await.insert(
            sid.clone(),
            Conn {
                tx,
                subjects: HashMap::new(),
            },
        );
        (sid, rx)
    }

    /// Subscribe `subject` on connection `sid` as `principal`. Runs the subject's host gate; on deny (or
    /// an unknown subject) pushes ONE opaque `error` mux frame and returns `Ok(())` — the connection is
    /// never torn down. On success, spawns the driver task (replacing any prior task for the same subject).
    /// `Err(NoSuchConn)` only if `sid` is unknown (a stale control POST after the stream dropped).
    pub async fn subscribe(
        &self,
        gw: &Gateway,
        sid: &str,
        subject: &str,
        principal: &Principal,
    ) -> Result<(), NoSuchConn> {
        // Resolve the feed (gate + snapshot) BEFORE taking the registry lock — the host read may await.
        let opened = open_subject(gw, principal, subject).await;
        let mut conns = self.conns.lock().await;
        let conn = conns.get_mut(sid).ok_or(NoSuchConn)?;
        let tx = conn.tx.clone();
        match opened {
            Ok(mut stream) => {
                let subject_owned = subject.to_string();
                let handle = tokio::spawn(async move {
                    while let Some(f) = stream.next().await {
                        let line = envelope(&subject_owned, &f.event, &f.data);
                        // Non-blocking push. Closed → the connection is gone, stop the task. Full → drop
                        // this frame and keep going: motion is fire-and-forget (rule 3), so a chatty
                        // subject can't wedge the connection (fairness over completeness for motion).
                        if let Err(err) = tx.try_send(line) {
                            if matches!(err, mpsc::error::TrySendError::Closed(_)) {
                                break;
                            }
                        }
                    }
                });
                // Replace any existing task for this subject (a re-subscribe restarts catch-up).
                if let Some(old) = conn.subjects.insert(subject.to_string(), handle) {
                    old.abort();
                }
            }
            Err(e) => {
                // Opaque per-subject error frame; the connection stays up (scope: deny ≠ kill).
                let _ = tx.try_send(error_envelope(subject, &e));
            }
        }
        Ok(())
    }

    /// Unsubscribe `subject` on `sid`: abort its driver task (releasing the bus subscription). A no-op if
    /// the subject isn't subscribed or the connection is gone (idempotent, matches the client refcount-zero).
    pub async fn unsubscribe(&self, sid: &str, subject: &str) {
        let mut conns = self.conns.lock().await;
        if let Some(conn) = conns.get_mut(sid) {
            if let Some(handle) = conn.subjects.remove(subject) {
                handle.abort();
            }
        }
    }

    /// Tear down connection `sid`: abort every subject task and forget it. Called by the SSE body's drop
    /// guard when the browser closes the stream (or it drops on reconnect) — so no orphaned bus tasks leak.
    pub async fn close(&self, sid: &str) {
        if let Some(conn) = self.conns.lock().await.remove(sid) {
            for (_, handle) in conn.subjects {
                handle.abort();
            }
        }
    }

    /// How many subject tasks are live on `sid` (0 if the connection is gone). Lets a test assert
    /// unsubscribe/close actually released the tasks (and their bus subscriptions).
    pub async fn subject_count(&self, sid: &str) -> usize {
        self.conns
            .lock()
            .await
            .get(sid)
            .map(|c| c.subjects.len())
            .unwrap_or(0)
    }
}

/// A control POST named a `sid` the hub doesn't know (the stream already dropped). The route maps it to
/// `404` — the client re-opens the stream and re-subscribes (its normal reconnect path).
#[derive(Debug)]
pub struct NoSuchConn;

/// Wrap one subject frame in the mux envelope: `{"sub":subject,"event":name,"data":<verbatim frame>}`.
/// `data` is spliced as raw JSON (the payload the dedicated route emits, byte-identical) — not re-encoded.
fn envelope(subject: &str, event: &str, data: &str) -> String {
    // `data` is already a JSON document; embed it verbatim so client fold logic is untouched.
    format!(
        "{{\"sub\":{},\"event\":{},\"data\":{}}}",
        json!(subject),
        json!(event),
        data
    )
}

/// The opaque per-subject error envelope (no oracle: unknown vs denied are the same to the client).
fn error_envelope(subject: &str, _e: &SubjectError) -> String {
    envelope(
        subject,
        "error",
        &json!({ "error": "not available" }).to_string(),
    )
}
