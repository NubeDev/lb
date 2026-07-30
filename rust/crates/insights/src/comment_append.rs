//! `append_comment` — the append-only thread write (insight-triage-scope.md).
//!
//! Two refusals, both BEFORE any write, so a rejected append leaves the existing thread exactly as
//! it was (the scope's "the oldest comment is still there after a refused write" assertion):
//!   1. the per-comment size cap (`validate_comment`), and
//!   2. the per-insight **count** cap — which **refuses**, and does NOT evict the oldest.
//!
//! That second one is the decision most likely to be "helpfully" reverted to match the occurrence
//! ring sitting beside it. It is deliberate: see `comment.rs`'s module doc (resolved decision 4).
//! If you are here to make this a ring, you are reverting a decision, not fixing an inconsistency.
//!
//! `seq` is host-assigned (previous max + 1, monotone per insight); `author` is host-stamped by the
//! service layer from the principal, never caller-supplied.

use lb_store::{write, Store};

use crate::comment::{Comment, MAX_COMMENTS_PER_INSIGHT, TABLE};
use crate::comments::comments;
use crate::error::InsightsError;

/// Append one comment to insight `insight_id` in workspace `ws`, returning the assigned `seq`.
///
/// Refuses (leaving the thread untouched) when `text` is empty/oversize or the thread already holds
/// [`MAX_COMMENTS_PER_INSIGHT`] comments. The caller must have already established that the insight
/// exists — the service layer does that so a comment on a missing insight errors like `ack` does.
// SCOPE: docs/scope/insights/insight-triage-scope.md §"Resolved decisions" (4 — refuse, never evict)
pub async fn append_comment(
    store: &Store,
    ws: &str,
    insight_id: &str,
    text: &str,
    author: &str,
    ts: u64,
) -> Result<u64, InsightsError> {
    crate::comment::validate_comment(text)?;

    // Read the existing thread FIRST: it gives both the count-cap check and the next `seq`, and it
    // means the cap refusal happens before any write.
    let existing = comments(store, ws, insight_id).await?;
    if existing.len() >= MAX_COMMENTS_PER_INSIGHT {
        return Err(InsightsError::BadInput(format!(
            "insight {insight_id} already has {} comments (the {MAX_COMMENTS_PER_INSIGHT} cap) — the thread is NOT truncated and nothing was deleted; a finding needing more discussion than this should become a work item",
            existing.len()
        )));
    }
    // Monotone per insight: previous max + 1. Read from the thread rather than the parent's `count`
    // (which is the raise accounting) — comments and firings are unrelated sequences.
    let seq = existing.iter().map(|c| c.seq).max().unwrap_or(0) + 1;

    let comment = Comment {
        seq,
        text: text.to_string(),
        author: author.to_string(),
        ts,
    };
    // The stored body carries `insight_id` (the filter the thread read uses) beside the comment's
    // own fields. Row id is `{insight_id}:{seq}` — stable, unique, and readable in a store dump.
    let mut body = serde_json::to_value(&comment)
        .map_err(|e| InsightsError::Store(lb_store::StoreError::Decode(e.to_string())))?;
    if let Some(obj) = body.as_object_mut() {
        obj.insert("insight_id".into(), serde_json::json!(insight_id));
    }
    write(store, ws, TABLE, &format!("{insight_id}:{seq}"), &body).await?;
    Ok(seq)
}
