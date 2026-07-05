//! `match_subs` — the raise-time matcher (insight-subscriptions-scope.md).
//!
//! A PURE function `(insight_view, subs) -> Vec<Intent>`. Called by the host's raise path after
//! the record write + occurrence append + bus event, inside the same raise handling. Each axis is
//! field equality / severity ordering / tag-subset; an empty filter matches all. A muted sub
//! still produces an intent (the notify state keeps accumulating so an unmute doesn't lose the
//! digest); the notify engine drops the delivery, not the accounting.
//!
//! **STUB**: the AND-filter / tag-subset algorithm body is deferred. This is the single
//! load-bearing pure function for subscriptions — see the punch-list.

use crate::intent::{Intent, IntentKind};
use crate::subscription::Subscription;

/// The matcher's read-only view of the raised insight — only the fields a sub filter touches.
/// Built by the host from the post-raise Insight record + its tag edges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InsightView<'a> {
    pub insight_id: &'a str,
    pub dedup_key: &'a str,
    pub severity: crate::severity::Severity,
    pub origin_ref: &'a str,
    /// The insight's tag facets — `tags.find`-style `{ k: v }`. The matcher's subset check reads
    /// this; the host materializes it from the tag graph before calling.
    pub tags: &'a std::collections::BTreeMap<String, String>,
    /// What kind of raise — passed through to the intent (drives the breakthrough check).
    pub kind: IntentKind,
}

/// Compute the intents a raise produces: one per matching sub. Pure — no I/O, no clock. The host
/// loads the workspace's subs (capped at the ws sub_cap) and calls this once per raise.
// SCOPE: docs/scope/insights/insight-subscriptions-scope.md §"The raise-time matcher"
pub fn match_subs<'a>(view: &InsightView<'a>, subs: &'a [Subscription]) -> Vec<Intent> {
    subs.iter()
        .filter(|sub| sub.dormant_reason.is_none() && filter_matches(&sub.filter, view))
        .map(|sub| Intent {
            sub_id: sub.id.clone(),
            insight_id: view.insight_id.to_string(),
            dedup_key: view.dedup_key.to_string(),
            severity: view.severity,
            kind: view.kind,
        })
        .collect()
}

/// AND every provided filter axis; all absent = "all insights". A muted sub STILL matches — the
/// notify state accumulates so an unmute doesn't lose the digest (the notify engine drops the
/// delivery, not the accounting). A dormant sub is excluded by the caller above.
fn filter_matches(filter: &crate::subscription::SubFilter, view: &InsightView<'_>) -> bool {
    if let Some(origin_ref) = &filter.origin_ref {
        if origin_ref != view.origin_ref {
            return false;
        }
    }
    if let Some(dedup_key) = &filter.dedup_key {
        if dedup_key != view.dedup_key {
            return false;
        }
    }
    if let Some(floor) = filter.severity_min {
        if !view.severity.at_least(floor) {
            return false;
        }
    }
    // Tag facet: the insight must carry EVERY (k, v) in the filter (subset check). Extra tags on
    // the insight don't disqualify it.
    filter
        .tags
        .iter()
        .all(|(k, v)| view.tags.get(k).is_some_and(|got| got == v))
}
