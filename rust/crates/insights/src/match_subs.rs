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
pub fn match_subs<'a>(_view: &InsightView<'a>, _subs: &'a [Subscription]) -> Vec<Intent> {
    // For each sub (skip dormant subs — they do not match):
    //   - origin_ref: sub.filter.origin_ref exact-equals view.origin_ref (or absent ⇒ any).
    //   - dedup_key:  sub.filter.dedup_key  exact-equals view.dedup_key  (or absent ⇒ any).
    //   - severity_min: view.severity.at_least(sub.filter.severity_min).
    //   - tags: every (k,v) in sub.filter.tags is in view.tags (subset check).
    //   - All provided axes must match (AND); all absent = "all insights" (empty filter matches).
    // A muted sub STILL produces an intent — the notify state accumulates so an unmute doesn't
    // lose the digest; the notify engine drops the delivery, not the accounting.
    todo!("insights: AND-filter + tag-subset matcher — SCOPE: subscriptions-scope.md §The raise-time matcher")
}
