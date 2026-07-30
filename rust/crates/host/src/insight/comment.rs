//! `insight_comment` — append one human note to an insight's thread, over its OWN capability
//! (insight-triage-scope.md).
//!
//! Gated on `mcp:insight.comment:call`. Like `assign`, this is a narrow verb rather than a slice of
//! a generic `insight.update`, so a producer holding only `mcp:insight.raise:call` cannot write
//! human triage state.
//!
//! `author` is **forced** to the principal's `sub` — the `ack.rs` host-stamp precedent. A caller
//! supplying `author: "user:someone-else"` is ignored, not refused: the field simply is not read
//! from the input, so there is no path by which a forged author reaches the store.

use std::sync::Arc;

use lb_auth::Principal;
use lb_mcp::authorize_tool;

use super::error::InsightSvcError;
use crate::boot::Node;

/// Append `text` to insight `id`'s thread in workspace `ws` as `principal`, returning the assigned
/// `seq`. Errors when the insight does not exist (like `ack`), when `text` is empty or oversize, or
/// when the thread is at its count cap — in every failing case the existing thread is untouched.
pub async fn insight_comment(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    id: &str,
    text: &str,
    ts: u64,
) -> Result<u64, InsightSvcError> {
    authorize_tool(principal, ws, "insight.comment").map_err(|_| InsightSvcError::Denied)?;

    // Establish the insight exists BEFORE writing a comment, so a note can never be stranded under
    // a parent that isn't there (the cascade in `delete` only reaches rows whose parent existed).
    // Same error shape `ack` gives, so "no such insight" reads identically across the act verbs.
    if lb_insights::get(&node.store, ws, id).await?.is_none() {
        return Err(InsightSvcError::BadInput(format!("no such insight: {id}")));
    }

    let seq = lb_insights::append_comment(&node.store, ws, id, text, principal.sub(), ts).await?;
    super::triage_event::publish_triage_event(node, ws, id, "comment").await;
    Ok(seq)
}
