//! `ServicePolicy` — the workspace's SLA contract row
//! (`docs/scope/insights/case-plane-scope.md`, the `service_policy` table).
//!
//! One row per contract, seeded by packs and editable in settings. A policy says, for the slice of
//! work its `match` selects: how long we have to **respond**, how long we have to **resolve**, in
//! which **calendar** those hours are counted, how long a **party** gets to answer a request, and
//! how long a `fixed` case stays hot enough that a re-fire reopens it instead of opening a new one.
//!
//! # Rule 10
//!
//! Nothing here names a pack, a rule, a site, an extension, or a category *value*. `PolicyMatch`
//! holds three **opaque strings** compared by equality. lb ships no vocabulary for any of them —
//! the workspace's `tag_vocab` row owns that, and this crate never reads it.
//!
//! # Prioritisation is a deadline, not a score
//!
//! There is no weight, no points, no "P1". A case's place in the queue is `due_at` ascending — a
//! wall-clock fact a human can argue with and a contract can be held to. That is why this record
//! carries hours and a calendar and nothing that could be summed into a number.

use serde::{Deserialize, Serialize};

use crate::calendar::Calendar;

/// The store table service policies live in. Workspace-scoped like every other row.
pub const POLICY_TABLE: &str = "service_policy";

/// The default hold-down window, in days.
///
/// **14 days, and the reasoning is the scope's:** a detection rule's own window measures *detection
/// cadence* — how often we are willing to be told — not how long a repair takes to prove itself. A
/// compressor that was "fixed" on Monday and short-cycles again on Thursday did not get fixed, and
/// a rule that fires daily would otherwise open a fresh case and lose that fact. A contract that
/// wants a different proof window edits its own policy row; nothing infers this from a rule.
pub const DEFAULT_HOLD_DOWN_DAYS: u32 = 14;

/// The default window a party gets to answer a request, in hours. A plain default (two calendar
/// days), not a decision from the scope — a policy row or the party's own `default_ask_window_h`
/// overrides it.
pub const DEFAULT_PARTY_WINDOW_H: u32 = 48;

/// Which work a policy applies to. Every field is an **opaque string** supplied as data; a `None`
/// field means "any". All three `None` is the workspace default, which matches everything.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyMatch {
    /// Site the case sits at, or `None` for any site.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site: Option<String>,
    /// Case category, or `None` for any category.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// Case severity, or `None` for any severity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity: Option<String>,
}

impl PolicyMatch {
    /// How many axes this match pins — `0..=3`. The workspace default is `0`.
    ///
    /// This is the *ordering* key (`policy.sla.list` returns most-specific first) and the ceiling
    /// of the *resolution* score in [`crate::policy_match`]. It is a property of the record, so it
    /// lives on the record.
    pub fn specificity(&self) -> u8 {
        self.site.is_some() as u8 + self.category.is_some() as u8 + self.severity.is_some() as u8
    }

    /// Whether this is the workspace default — the empty match that catches everything the
    /// specific rows did not.
    pub fn is_default(&self) -> bool {
        self.specificity() == 0
    }
}

/// One SLA contract. Hours are **business hours in [`Self::calendar`]**, never wall hours (unless
/// the calendar is [`Calendar::Always`], which is the same thing).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServicePolicy {
    /// Stable id, unique within the workspace. Also the deterministic tie-break in resolution.
    pub id: String,
    /// Human label for the settings list.
    #[serde(default)]
    pub name: String,
    /// Which work this policy governs. Empty ⇒ the workspace default.
    ///
    /// `match` is a Rust keyword, so the field is a raw identifier; the wire name is `match`.
    #[serde(default, rename = "match")]
    pub r#match: PolicyMatch,
    /// Business hours from case open to the first response.
    ///
    /// Deliberately **not** `#[serde(default)]`: a policy row without a response deadline is not a
    /// policy, and defaulting it to zero would make every case instantly breached while defaulting
    /// it to some number would invent a contract nobody agreed to. A malformed row must fail to
    /// decode, loudly, rather than quietly govern work.
    pub respond_h: u32,
    /// Business hours from case open to resolution. Not defaulted, for the same reason.
    pub resolve_h: u32,
    /// The calendar those hours are counted in. Absent ⇒ [`Calendar::Always`] (24x7).
    #[serde(default)]
    pub calendar: Calendar,
    /// How long a party has to answer a request, in hours.
    #[serde(default = "default_party_window_h")]
    pub party_window_h: u32,
    /// How long after a `fixed` resolution a re-fire reopens the case instead of opening a new one.
    #[serde(default = "default_hold_down_days")]
    pub hold_down_days: u32,
}

fn default_party_window_h() -> u32 {
    DEFAULT_PARTY_WINDOW_H
}

fn default_hold_down_days() -> u32 {
    DEFAULT_HOLD_DOWN_DAYS
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The scope's decision, pinned: a row that says nothing about hold-down holds down for 14
    /// days. If someone "tidies" this to track a rule's window, this test is the argument.
    #[test]
    fn hold_down_defaults_to_fourteen_days() {
        let p: ServicePolicy =
            serde_json::from_str(r#"{"id":"default","respond_h":4,"resolve_h":24}"#).unwrap();
        assert_eq!(p.hold_down_days, 14);
        assert_eq!(p.party_window_h, DEFAULT_PARTY_WINDOW_H);
        assert_eq!(p.calendar, Calendar::Always);
        assert!(p.r#match.is_default());
        assert_eq!(p.name, "");
    }

    /// A row missing a deadline does not decode — it must not silently become "0 hours" (instantly
    /// breached) or some invented default.
    #[test]
    fn a_row_without_deadlines_refuses_to_decode() {
        assert!(serde_json::from_str::<ServicePolicy>(r#"{"id":"x","resolve_h":24}"#).is_err());
        assert!(serde_json::from_str::<ServicePolicy>(r#"{"id":"x","respond_h":4}"#).is_err());
    }

    /// Specificity counts pinned axes, and `match` is the wire name.
    #[test]
    fn specificity_counts_pinned_axes() {
        let m: PolicyMatch =
            serde_json::from_str(r#"{"site":"s1","severity":"critical"}"#).unwrap();
        assert_eq!(m.specificity(), 2);
        assert!(!m.is_default());
        assert_eq!(PolicyMatch::default().specificity(), 0);
        assert!(PolicyMatch::default().is_default());

        let p = ServicePolicy {
            id: "p".into(),
            name: "n".into(),
            r#match: m,
            respond_h: 1,
            resolve_h: 2,
            calendar: Calendar::Always,
            party_window_h: 48,
            hold_down_days: 14,
        };
        let wire = serde_json::to_value(&p).unwrap();
        assert!(wire.get("match").is_some(), "wire name is `match`");
        // An unset axis is omitted rather than written as null.
        assert!(wire["match"].get("category").is_none());
    }
}
