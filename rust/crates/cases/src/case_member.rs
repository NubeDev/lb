//! `CaseMember` — the citation edge "this case covers this insight" (case-plane scope).
//!
//! **A separate table, not an array on the case.** A storm is hundreds of detections; folding them
//! into the case record would make every lane read carry a payload only the drawer opens, and the
//! roster's job is to stay small enough to list.
//!
//! The pair `(case_id, insight_id)` is unique, and the invariant across the table is stronger:
//! **every open insight is in exactly one OPEN case.** [`crate::member_add`] refuses to break it;
//! [`crate::merge`] is the verb that MOVES members between cases.
//!
//! [`CaseMember::human_placed`] is the reactors' stop sign. A person who moved a detection into a
//! case made a judgement the machine cannot see; no reactor may move that member again, ever.

use serde::{Deserialize, Serialize};

/// The store table membership rows live in. `case_id` and `insight_id` are both `data` fields, so
/// both directions of the read ("this case's members", "this insight's case") are single-field
/// equality filters rather than a scan.
pub const TABLE: &str = "case_member";

/// WHY this insight is in this case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemberRole {
    /// The finding the case is about. Exactly one per case, and it is the case's
    /// [`crate::Case::primary_insight`].
    Primary,
    /// A finding the primary explains — a downstream symptom, or the verdict record that named the
    /// root cause.
    Explained,
    /// A finding beneath the primary in the equipment tree (topology grouping).
    Child,
    /// One of many simultaneous detections of one incident (storm grouping).
    Storm,
    /// A detection folded in by [`crate::merge`].
    Duplicate,
}

/// One `(case, insight)` citation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaseMember {
    /// The case this row belongs to.
    pub case_id: String,
    /// The insight cited.
    pub insight_id: String,
    /// Why (see [`MemberRole`]).
    pub role: MemberRole,
    /// Who placed it — `system:reactor` for the grouping reactors, a `user:`/`team:` subject for a
    /// human gesture.
    pub added_by: String,
    /// **A person put this here.** No reactor may move or remove it — see the module doc. Set by
    /// the host verbs (`case.open`, `case.merge`, `case.split`), never by the reactors.
    #[serde(default)]
    pub human_placed: bool,
    /// Logical timestamp the membership was written.
    pub ts: u64,
}

/// The stable row id for a membership: `{case_id}:{insight_id}`. Unique by construction (so a
/// re-add upserts rather than duplicating) and readable in a store dump — the same shape the
/// insight comment thread's `{insight_id}:{seq}` uses.
pub fn member_id(case_id: &str, insight_id: &str) -> String {
    format!("{case_id}:{insight_id}")
}
