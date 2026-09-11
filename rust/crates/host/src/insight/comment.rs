//! `insight_comment` — append one human note to an insight's thread, over its OWN capability
//! (insight-triage-scope.md), **delegating to the case that owns the work** (case-plane scope,
//! resolved decision 5).
//!
//! Gated on `mcp:insight.comment:call`. Like `assign`, this is a narrow verb rather than a slice of
//! a generic `insight.update`, so a producer holding only `mcp:insight.raise:call` cannot write
//! human triage state. Delegating changes none of that: same cap, same verb, same `{ seq }` return.
//!
//! **Where the note lands changed.** A note is a fact about the WORK ("attended, replaced the
//! sensor, waiting on the client's PO"), so the case's history is now its home — which is also what
//! puts the discussion, the transitions and the contractor replies in ONE chronological list
//! instead of two tabs. The insight's own thread keeps receiving the note as an **echo**, exactly as
//! `assigned_to` does, because it is what `insight.get` composes into the drawer and what the
//! thread's count cap is measured against. The returned `seq` is the insight thread's, unchanged.
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

    // The insight thread FIRST. It holds the stricter bounds — a 4 KB per-note cap and a 200-note
    // count cap that REFUSES rather than evicting — so running it first means a note the thread
    // would reject never reaches the case history either. The reverse order would let an oversize
    // note land on the case and then fail the call, which is the one outcome a refusal must not
    // produce.
    let seq = lb_insights::append_comment(&node.store, ws, id, text, principal.sub(), ts).await?;

    // The case is where the conversation lives. Grouping the insight if it has none is the same
    // call the raise path makes, so commenting on an un-backfilled finding heals it.
    match crate::case::group_insight(node, ws, id, ts).await {
        Ok(case_id) => {
            if let Err(e) =
                lb_cases::comment(&node.store, ws, &case_id, text, principal.sub(), ts).await
            {
                // Best-effort, deliberately: the note is already durable on the insight, so a case
                // history one row behind is a stale projection — never a reason to tell a person
                // their note was rejected when it was not.
                tracing::warn!(ws, id, %case_id, error = %e, "case comment echo not written");
            }
        }
        Err(e) => {
            tracing::warn!(ws, id, error = %e, "insight comment: case not resolved; note kept on the insight")
        }
    }

    super::triage_event::publish_triage_event(node, ws, id, "comment").await;
    Ok(seq)
}
