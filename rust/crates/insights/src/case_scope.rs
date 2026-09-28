//! `CaseScope` — the host-resolved case lens on `insight.list`: narrow and count detections by their
//! CASE's stage, and by the case queue's filters (case-plane scope; the Detections roster).
//!
//! **This crate still does not know what a case is.** The host reads the case plane and hands in
//! plain strings: every known case id with an opaque stage label, the ids that pass the queue's
//! filters, and which labels the reader picked. The only fact used about a detection is its own
//! `case_id` echo. That is the `tag_allow` pattern: the host resolves, this crate filters and counts.
//!
//! **The stage filter is left out of its own count**, as `status` is left out of the status tally: the
//! reply is the per-stage breakdown, so narrowing by stage first would zero every other stage's
//! number. The case-filter allowlist is NOT left out: the counts describe the filtered set.
//!
//! One responsibility: the case lens's shape and its per-row questions.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::insight::Insight;

/// The stage label of a detection with no case, or whose case id the host did not return (a case
/// deleted after the echo was written). Such a detection has not been picked up by anyone.
pub const NO_CASE: &str = "none";

/// The case lens, set by the host from server state (`ListFilter::case`, never read from the wire).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CaseScope {
    /// Every case id the host knows, with its stage label.
    pub stage_of: HashMap<String, String>,
    /// `Some` ⇒ only detections whose case id is in the set (the queue's filters). A detection with
    /// no case never passes: a case filter asks about a case.
    pub allow: Option<HashSet<String>>,
    /// `Some` ⇒ only detections whose stage label is in the set; [`NO_CASE`] is a legal member.
    pub stages: Option<HashSet<String>>,
    /// The stage labels in the order a `case_stage` sort ranks them (the host's workflow order). A
    /// label missing here, and [`NO_CASE`], has no rank and sorts last.
    pub stage_order: Vec<String>,
}

impl CaseScope {
    /// The detection's stage label.
    pub fn stage_of(&self, insight: &Insight) -> &str {
        insight
            .case_id
            .as_ref()
            .and_then(|c| self.stage_of.get(c))
            .map(String::as_str)
            .unwrap_or(NO_CASE)
    }

    /// Does the detection pass the case filters' allowlist?
    pub fn allows(&self, insight: &Insight) -> bool {
        match &self.allow {
            None => true,
            Some(ids) => insight.case_id.as_ref().is_some_and(|c| ids.contains(c)),
        }
    }

    /// Does the detection pass the stage filter?
    pub fn in_stages(&self, insight: &Insight) -> bool {
        match &self.stages {
            None => true,
            Some(set) => set.contains(self.stage_of(insight)),
        }
    }

    /// The case ids whose stage is in `stages`, for the SQL form of [`Self::in_stages`].
    pub(crate) fn ids_in_stages(&self, stages: &HashSet<String>) -> Vec<String> {
        self.stage_of
            .iter()
            .filter(|(_, s)| stages.contains(s.as_str()))
            .map(|(id, _)| id.clone())
            .collect()
    }
}

/// Count `rows` per stage label. Every label the host knows appears, zero included, so a reader
/// never has to tell "none of these" from "not counted".
pub(crate) fn stage_tally<'a>(
    scope: &CaseScope,
    rows: impl IntoIterator<Item = &'a Insight>,
) -> BTreeMap<String, u64> {
    let mut out: BTreeMap<String, u64> = scope
        .stage_of
        .values()
        .map(|s| (s.clone(), 0))
        .chain([(NO_CASE.to_string(), 0)])
        .collect();
    for row in rows {
        *out.entry(scope.stage_of(row).to_string()).or_default() += 1;
    }
    out
}
