//! `insight_assign` — set / re-assign / un-assign an insight's owner, over its OWN capability
//! (insight-triage-scope.md).
//!
//! Gated on `mcp:insight.assign:call`, deliberately NOT on a generic `insight.update`: one cap for
//! "change any field" would hand every producer holding `mcp:insight.raise:call` the power to
//! rewrite human triage state, and the deny path would stop being expressible. A producer grant buys
//! **zero** triage write power — that separation is the reason this verb exists as its own verb.
//!
//! The assignee is necessarily caller-supplied (you assign to someone else), so it is **validated,
//! not trusted** — unlike the comment `author`, which is host-stamped. See `assignee.rs` for the
//! membership rule and the opacity contract it holds.
//!
//! Bulk is bounded-synchronous: a roster with checkboxes is useless without "assign these 12 to me",
//! and per-item results mean 12 silent failures can never read as a green toast.

use std::sync::Arc;

use lb_auth::Principal;
use lb_mcp::authorize_tool;

use super::assignee::validate_assignee;
use super::error::InsightSvcError;
use crate::boot::Node;

/// The most ids one bulk assign accepts. Exceeding it is an EXPLICIT error, never a silent
/// truncation to the first 100 (the no-silent-caps rule — resolved decision 6).
pub const MAX_BULK_ASSIGN: usize = 100;

/// One id's outcome in a bulk assign. `ok: false` carries the reason, so a UI can surface the
/// failures rather than folding them into a success.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AssignResult {
    pub id: String,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Assign every id in `ids` to `assignee` (`None` un-assigns) in workspace `ws` as `principal`.
///
/// The assignee is validated ONCE up front — it is the same subject for every id, so re-reading
/// membership per item would be 100 identical store reads for one answer. An invalid assignee
/// therefore fails the whole call (nothing is written), which is right: it is a caller error about
/// the request, not a per-item outcome. Per-item results carry only per-item failures (a missing
/// insight, a store error).
pub async fn insight_assign(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    ids: &[String],
    assignee: Option<&str>,
) -> Result<Vec<AssignResult>, InsightSvcError> {
    authorize_tool(principal, ws, "insight.assign").map_err(|_| InsightSvcError::Denied)?;

    if ids.is_empty() {
        return Err(InsightSvcError::BadInput(
            "no insight ids given — pass `id` or a non-empty `ids`".into(),
        ));
    }
    if ids.len() > MAX_BULK_ASSIGN {
        return Err(InsightSvcError::BadInput(format!(
            "{} ids exceeds the {MAX_BULK_ASSIGN}-id bulk cap — nothing was assigned; split the call \
             (the cap is reported rather than silently truncating your request)",
            ids.len()
        )));
    }
    if let Some(a) = assignee {
        validate_assignee(&node.store, ws, a).await?;
    }

    let mut results = Vec::with_capacity(ids.len());
    for id in ids {
        match lb_insights::assign(&node.store, ws, id, assignee).await {
            Ok(_) => {
                results.push(AssignResult {
                    id: id.clone(),
                    ok: true,
                    error: None,
                });
                // Live-UI motion on the EXISTING insight subject — a new subject for triage would
                // fragment a stream the roster already holds open (insight-triage-scope.md).
                super::triage_event::publish_triage_event(node, ws, id, "assign").await;
            }
            Err(e) => results.push(AssignResult {
                id: id.clone(),
                ok: false,
                error: Some(e.to_string()),
            }),
        }
    }
    Ok(results)
}
