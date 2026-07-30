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

use std::collections::{HashMap, HashSet};

use crate::intent::{Intent, IntentKind};
use crate::subscription::{Subscription, ASSIGNEE_ME};

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
    /// Who owns the finding (`insight-assignee-notify-scope.md`) — the triage plane's match axis.
    /// `None` = unassigned, which never matches a filter that names an assignee.
    pub assigned_to: Option<&'a str>,
    /// What kind of raise — passed through to the intent (drives the breakthrough check).
    pub kind: IntentKind,
}

/// Per-sub-owner subject expansion: for each subscription owner, the set of subjects they count as —
/// their own `sub` plus every `team:` they belong to. Built by the HOST (team membership is a store
/// read; this crate stays pure and I/O-free) and consulted only to resolve
/// [`ASSIGNEE_ME`](crate::subscription::ASSIGNEE_ME).
///
/// Resolved at fire time, never stored on the subscription: an expansion frozen at create time
/// silently stops matching when the owner joins or leaves a team, and a subscription that has quietly
/// gone deaf is worse than one that costs a read.
pub type OwnerSubjects = HashMap<String, HashSet<String>>;

/// Compute the intents a raise produces: one per matching sub. Pure — no I/O, no clock. The host
/// loads the workspace's subs (capped at the ws sub_cap) and calls this once per raise.
///
/// `owner_subjects` resolves [`ASSIGNEE_ME`] per sub owner (see [`OwnerSubjects`]); pass an empty map
/// when no subscription filters on an assignee.
// SCOPE: docs/scope/insights/insight-subscriptions-scope.md §"The raise-time matcher"
// SCOPE: docs/scope/insights/insight-assignee-notify-scope.md §"Intent / approach" (capability A)
pub fn match_subs<'a>(
    view: &InsightView<'a>,
    subs: &'a [Subscription],
    owner_subjects: &OwnerSubjects,
) -> Vec<Intent> {
    subs.iter()
        .filter(|sub| {
            sub.dormant_reason.is_none()
                && filter_matches(&sub.filter, view, &sub.owner, owner_subjects)
        })
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
fn filter_matches(
    filter: &crate::subscription::SubFilter,
    view: &InsightView<'_>,
    owner: &str,
    owner_subjects: &OwnerSubjects,
) -> bool {
    if let Some(assignee) = &filter.assignee {
        if !assignee_matches(assignee, view.assigned_to, owner, owner_subjects) {
            return false;
        }
    }
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

/// Does a finding owned by `assigned_to` satisfy a filter asking for `want`?
///
/// An **unassigned** finding never matches a filter that names an assignee — "anything assigned to my
/// crew" must not mean "anything, including what nobody owns". `"me"` expands to the SUB OWNER's
/// subject set (their own sub + their teams), which is why a team-assigned finding reaches every crew
/// member's subscription; an owner missing from the map resolves to just their own sub, so a
/// team-read hiccup narrows the match rather than widening it.
///
/// Shared by the raise-time matcher and the assign-time notifier, so both answer "is this mine?"
/// identically — the one place that question is decided.
// SCOPE: docs/scope/insights/insight-assignee-notify-scope.md §"Resolved decisions" (6)
pub fn assignee_matches(
    want: &str,
    assigned_to: Option<&str>,
    owner: &str,
    owner_subjects: &OwnerSubjects,
) -> bool {
    let Some(actual) = assigned_to else {
        return false;
    };
    if want == ASSIGNEE_ME {
        return match owner_subjects.get(owner) {
            Some(subjects) => subjects.contains(actual),
            None => actual == owner,
        };
    }
    want == actual
}
