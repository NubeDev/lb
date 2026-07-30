//! The triage plane's live-UI motion — one fire-and-forget publish on the **existing** insight
//! event subject (insight-triage-scope.md §"Live feed").
//!
//! Both triage verbs emit here so an open roster re-renders (a new owner column value, a comment
//! badge). Deliberately NOT a new subject: the page already holds `ws/{ws}/insight/events` open, and
//! a subject per feature is the SSE-pool lesson (one stream per surface, not per feature).
//!
//! State vs motion (README §3.3): the store already holds the truth by the time this runs, so a
//! failed publish is a stale live view that the next read heals — never a failed assign/comment.

use std::sync::Arc;

use lb_insights::{EventKind, RaiseEvent};

use crate::boot::Node;

/// Publish the triage event for `id`. `kind_name` is `"assign"` or `"comment"`.
///
/// Re-reads the insight to fill the lite payload's status/severity/count. That is one extra read on
/// a low-frequency human verb, and it buys a payload identical in shape to the raise event — so a
/// live UI has exactly one event shape to handle rather than a special triage case.
pub async fn publish_triage_event(node: &Arc<Node>, ws: &str, id: &str, kind_name: &str) {
    let Ok(Some(insight)) = lb_insights::read_insight(&node.store, ws, id).await else {
        return;
    };
    let event = RaiseEvent {
        kind: match kind_name {
            "comment" => EventKind::Comment,
            _ => EventKind::Assign,
        },
        id: insight.id.clone(),
        dedup_key: insight.dedup_key.clone(),
        status: insight.status,
        severity: insight.severity,
        count: insight.count,
        ts: insight.last_ts,
    };
    if let Ok(payload) = serde_json::to_vec(&event) {
        // The workspace-native subject directly (NOT the walled ext bus path) — the raise precedent.
        let _ = lb_bus::publish(&node.bus, ws, "insight/events", &payload).await;
    }
}
