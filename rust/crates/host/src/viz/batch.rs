//! `viz.query_batch {panels[], now?, cache?}` — the batch fan-in verb (dashboard-query-acceleration
//! scope, slice 3). One HTTP round-trip resolves a whole board's panels **concurrently server-side**,
//! killing the browser's HTTP/1.1 connection ceiling (24 tiles → 24 POSTs behind a ~6-connection cap →
//! ~4 serial waves) without adding a new privilege.
//!
//! Shape: `{ results: [ <the same `{frames, rows}` a single `viz.query` returns> | { status, message } ] }`,
//! index-aligned with the request `panels[]` (each frame also carries its own `refId`, so a caller can
//! read its slice by index OR refId).
//!
//! Invariants (the scope's slice-3 contract):
//!   - **Same gate, no new cap.** It rides `mcp:viz.query:call` — a fan-in of the same authorized verb,
//!     not a new privilege. The outer dispatcher aliases it (`gate_tool_for`); `tool.rs` re-checks it.
//!   - **Rides the gateway cache.** Each panel resolves through [`crate::cache::dispatch`] for `viz.query`
//!     — the SAME `subject_scoped` cached path a lone `viz.query` takes (scope open-Q4: the cache wraps
//!     the resolver, not the batch verb), so a batch of mostly-warm tiles is a handful of computes and
//!     concurrent identical batches single-flight per panel.
//!   - **Per-item partial failure.** One bad panel returns its own `{status:"error"|"denied"}`; the rest
//!     resolve. A dashboard never blanks because one tile's SQL is wrong.
//!   - **Bounded & synchronous — NOT a job.** A hard cap of [`MAX_PANELS`] (over-cap ⇒ `BadInput`, the UI
//!     chunks) and a per-frame row cap bound the work; a semaphore bounds the concurrent fan-out. It is
//!     always-fast fan-in, so it stays a normal synchronous call (SCOPE-WRITTING §6.1).

use std::sync::Arc;

use futures::future::join_all;
use lb_auth::Principal;
use lb_mcp::ToolError;
use serde_json::{json, Value};
use tokio::sync::Semaphore;

use super::error::VizError;
use crate::boot::Node;

/// The hard per-batch panel cap (scope open-Q2). Covers every real board; a larger one is chunked by the
/// UI into multiple batches — still ≪ one round-trip per tile. Over-cap ⇒ `BadInput` (never a silent
/// truncation that reads as "resolved everything").
const MAX_PANELS: usize = 64;

/// The max panels resolved concurrently. Bounds the fan-out so one batch can't open 64 datasource
/// connections at once; the per-source warm pool + per-query timeout (federation-pool-cache) contain a
/// slow panel, and a slow panel returns its own error without wedging its siblings.
const MAX_CONCURRENCY: usize = 16;

/// Resolve a batch of panels. `input` is the verb args (`{panels:[…], now?, cache?}`); the return is
/// `{results:[…]}`, one entry per input panel, in order.
pub async fn viz_query_batch(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    input: &Value,
    depth: u32,
) -> Result<Value, VizError> {
    let panels = input
        .get("panels")
        .and_then(Value::as_array)
        .ok_or_else(|| VizError::BadInput("viz.query_batch requires `panels: [...]`".into()))?;

    // Bounded, synchronous: refuse an over-cap batch rather than fan out unboundedly (the UI chunks).
    if panels.len() > MAX_PANELS {
        return Err(VizError::BadInput(format!(
            "viz.query_batch accepts at most {MAX_PANELS} panels ({} sent) — chunk the request",
            panels.len()
        )));
    }

    // `now`/`cache` are batch-level defaults, applied to every panel exactly as a single `viz.query`
    // carries them (a per-panel spec that already names its own is untouched — the single-panel input we
    // build only adds the batch-level ones alongside the panel).
    let now = input.get("now").and_then(Value::as_u64).unwrap_or(0);
    let cache = input.get("cache").cloned();

    // Resolve each panel through the SHARED cached single-panel path, concurrently, bounded. Cooperative
    // concurrency (join_all on one task) is right for I/O-bound resolves (DB round trips); the semaphore
    // caps the in-flight count.
    let sem = Arc::new(Semaphore::new(MAX_CONCURRENCY));
    let futures = panels.iter().map(|panel| {
        let sem = Arc::clone(&sem);
        let cache = cache.clone();
        async move {
            // A panel slot is never contended for a permit we can't get (the semaphore has no other
            // borrower and is never closed), so `acquire` cannot error here.
            let _permit = sem.acquire().await.expect("batch semaphore open");
            resolve_one_panel(node, principal, ws, panel, now, cache.as_ref(), depth).await
        }
    });
    let results: Vec<Value> = join_all(futures).await;

    Ok(json!({ "results": results }))
}

/// Resolve ONE panel through the same gateway-cached path a lone `viz.query` takes, mapping any error to
/// a per-item result so one bad panel never fails the batch. Builds the single-`viz.query` input shape
/// (`{panel, now, cache?}`) and hands it to [`crate::cache::dispatch`] under `viz.query` at the caller's
/// depth — so the `subject_scoped` gateway cache, the capability fingerprint, and the quantiser all apply
/// per panel, identically to the non-batch path.
async fn resolve_one_panel(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    panel: &Value,
    now: u64,
    cache: Option<&Value>,
    depth: u32,
) -> Value {
    let mut per_panel = json!({ "panel": panel, "now": now });
    if let (Value::Object(map), Some(cache)) = (&mut per_panel, cache) {
        map.insert("cache".into(), cache.clone());
    }

    match crate::cache::dispatch(node, principal, ws, "viz.query", &per_panel, depth).await {
        // The serialised `{frames, rows}` (or `{steps, rows}`) a single `viz.query` returns; hand it back
        // verbatim as this panel's slice. A denied TARGET is already an empty frame inside the resolver,
        // so an `Ok` here is a fully-resolved panel (some frames may be empty/denied — that is honest).
        Ok(json_str) => serde_json::from_str::<Value>(&json_str)
            .unwrap_or_else(|_| error_item("bad frame json")),
        // A whole-panel failure (malformed panel shape, or an operational error). Per-item — never fails
        // the batch. A `Denied`/`NotFound` stays OPAQUE (no message), matching the resolver's deny
        // contract; a `BadInput`/`Extension`/operational error surfaces its message to this same
        // authorized caller (their own request being validated).
        Err(ToolError::Denied) | Err(ToolError::NotFound) => json!({ "status": "denied" }),
        Err(ToolError::BadInput(m)) | Err(ToolError::Extension(m)) => error_item(&m),
        Err(e) => error_item(&e.to_string()),
    }
}

/// A per-item error result — the same `{status:"error", message}` shape a bad single-tile query returns,
/// so the UI renders a batched error identically to a non-batched one (no new empty-state).
fn error_item(message: &str) -> Value {
    json!({ "status": "error", "message": message })
}
