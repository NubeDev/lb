//! `viz.query_batch`, **streamed** — the same fan-in as `batch.rs`, but each panel's result is yielded
//! the moment it resolves instead of after the slowest sibling (dashboard-query-acceleration scope,
//! slice 4: progressive first paint). One authorization, one round-trip, one server-side concurrent
//! fan-out — and a board that paints tile by tile.
//!
//! What is SHARED with the synchronous verb (and must stay shared — the two are one contract):
//!   - the input shape + cap (`parse_batch`), the `now`/`cache` batch defaults;
//!   - the per-panel resolver (`resolve_one_panel`): the same `subject_scoped` gateway cache, the same
//!     capability fingerprint, the same per-item `{status:"error"|"denied"}` partial failure;
//!   - the concurrency bound (`batch_semaphore`).
//!
//! What differs: the return is a [`Stream`] of [`BatchItem`]s in **completion order**, each tagged with
//! its request index (the caller re-aligns by `index`; nothing is implied by arrival order). It is a
//! plain in-memory stream — the gateway route turns it into an NDJSON body; no bus, no job, no new
//! capability (`mcp:viz.query:call`, checked here exactly as `tool.rs` does for the batch verb).
//!
//! Owned inputs on purpose: the stream outlives the request handler's borrows.

use std::sync::Arc;

use futures::stream::{FuturesUnordered, Stream, StreamExt};
use lb_auth::Principal;
use lb_mcp::ToolError;
use serde_json::Value;

use super::authorize::authorize_viz;
use super::batch::{batch_semaphore, parse_batch, resolve_one_panel};
use super::error::VizError;
use crate::boot::Node;

/// One resolved panel: the index it had in the request's `panels[]` and the same `{frames, rows}` (or
/// `{status, message}`) value the synchronous batch places at that index.
#[derive(Debug, Clone)]
pub struct BatchItem {
    pub index: usize,
    pub result: Value,
}

/// Open the streamed batch. Authorizes ONCE (`mcp:viz.query:call`, the batch's aliased gate — a
/// fan-in of the same read, no new privilege), validates the input, and returns the stream. Errors
/// before the first item are the same `ToolError`s the verb raises (`Denied` opaque, `BadInput` with
/// its message); a per-panel failure never errors the stream — it arrives as that panel's item.
///
/// `depth` is the dispatch depth the caller sits at (`0` for the gateway route — the outermost entry,
/// so the gateway cache participates per panel exactly as it does for the verb).
pub fn viz_query_batch_stream(
    node: Arc<Node>,
    principal: Principal,
    ws: String,
    input: Value,
    depth: u32,
) -> Result<impl Stream<Item = BatchItem> + Send + 'static, ToolError> {
    authorize_viz(&principal, &ws, "viz.query").map_err(to_tool)?;
    let batch = parse_batch(&input).map_err(to_tool)?;

    let sem = batch_semaphore();
    let now = batch.now;
    let cache = Arc::new(batch.cache);

    let futures: FuturesUnordered<_> = batch
        .panels
        .into_iter()
        .enumerate()
        .map(|(index, panel)| {
            let node = Arc::clone(&node);
            let principal = principal.clone();
            let ws = ws.clone();
            let sem = Arc::clone(&sem);
            let cache = Arc::clone(&cache);
            async move {
                // Never contended for a permit we can't get (the semaphore is private to this batch
                // and never closed), so `acquire` cannot error.
                let _permit = sem.acquire().await.expect("batch semaphore open");
                let result = resolve_one_panel(
                    &node,
                    &principal,
                    &ws,
                    &panel,
                    now,
                    cache.as_ref().as_ref(),
                    depth,
                )
                .await;
                BatchItem { index, result }
            }
        })
        .collect();

    Ok(futures.boxed())
}

/// Same mapping `tool.rs` applies: denials opaque, bad input carries its message.
fn to_tool(e: VizError) -> ToolError {
    match e {
        VizError::Denied => ToolError::Denied,
        VizError::BadInput(m) => ToolError::BadInput(m),
    }
}
