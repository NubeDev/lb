//! `ingest_via_bridge` — the host composition that turns the installed `github-bridge` wasm extension
//! into the workflow's inbound edge (github-bridge scope, the S6 `github-bridge` deferral resolved).
//!
//! The bridge is a PURE-transform Tier-1 guest: it normalizes a raw GitHub webhook payload into the
//! canonical `{ issue_id, payload, ts }` triple, but it CANNOT write the inbox itself — the stable WIT
//! world imports only `host.log`, there is no host-tool-call import (by design, README §11.2). So the
//! HOST composes the two seams that already exist:
//!   1. `lb_mcp::call("github-bridge.normalize", raw)` — the sandboxed transform (capability-gated,
//!      workspace-first, like any tool call);
//!   2. `ingest_issue(normalized)` — the must-deliver inbox write (the host owns the store/caps seam).
//!
//! Two independent gates therefore apply: `mcp:github-bridge.normalize:call` on the transform and
//! `mcp:workflow.ingest_issue:call` on the write. Neither is widened here — both run under `principal`.
//! The bridge being a swappable artifact is the whole point: a GitLab/Gitea bridge sharing the same
//! `{issue_id, payload, ts}` output contract drops in without touching this host helper.

use lb_auth::Principal;
use lb_inbox::Item;
use lb_mcp::{call, ToolError};
use serde::Deserialize;

use super::error::WorkflowError;
use super::ingest::ingest_issue;
use crate::boot::Node;

/// The canonical triple the `github-bridge.normalize` tool returns — the only contract between the
/// (swappable) bridge artifact and this host edge. Mirrors the guest's `Normalized` shape.
#[derive(Deserialize)]
struct Normalized {
    issue_id: String,
    payload: String,
    ts: u64,
}

/// The MCP-qualified name of the installed bridge's normalize tool.
const NORMALIZE_TOOL: &str = "github-bridge.normalize";

/// Ingest a raw GitHub webhook `raw_json` for workspace `ws` as `principal`: normalize it through the
/// installed `github-bridge` wasm extension, then write the canonical issue to the `triage` inbox via
/// `ingest_issue`. Idempotent on the normalized `issue_id` (the inbox upserts on `(channel, id)`), so a
/// re-delivered webhook still produces one item. Returns the stored `Item`.
///
/// Errors map the bridge's tool error onto `WorkflowError`: a `Denied` from either gate stays opaque; a
/// malformed payload surfaces as the transform's `bad-input` (an extension error, a distinguishable
/// client fault). The bridge must be installed in `ws` first (`install_from_registry`) — an absent tool
/// is `Denied`-opaque, never an existence signal.
pub async fn ingest_via_bridge(
    node: &Node,
    principal: &Principal,
    ws: &str,
    raw_json: &str,
) -> Result<Item, WorkflowError> {
    // 1. The sandboxed transform — capability-gated (`mcp:github-bridge.normalize:call`), workspace-first.
    //    A `Denied`/`NotFound` stays opaque (an un-installed or un-granted bridge leaks nothing); a
    //    transform fault carries through as `Bridge`.
    let normalized_json = call(
        &node.registry,
        &node.bus,
        principal,
        ws,
        NORMALIZE_TOOL,
        raw_json,
    )
    .await
    .map_err(tool_err)?;
    let n: Normalized = serde_json::from_str(&normalized_json)
        .map_err(|e| WorkflowError::Bridge(format!("bridge output: {e}")))?;

    // 2. The must-deliver inbox write — its OWN gate (`mcp:workflow.ingest_issue:call`) runs inside.
    ingest_issue(&node.store, principal, ws, &n.issue_id, &n.payload, n.ts).await
}

/// Map the bridge tool-call error onto the workflow error: `Denied`/`NotFound` stay opaque (no
/// existence signal for an un-installed/un-granted bridge); a transform fault is a `Bridge` error.
fn tool_err(e: ToolError) -> WorkflowError {
    match e {
        ToolError::Denied | ToolError::NotFound => WorkflowError::Denied,
        ToolError::Extension(m) | ToolError::BadInput(m) => WorkflowError::Bridge(m),
    }
}
