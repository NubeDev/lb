//! The **party** record — an external actor a case can ask something of: a contractor, an FM, the
//! client, an FMS vendor (case-plane scope, wave 2).
//!
//! **Rule 10 holds absolutely here.** A party is DATA. Nothing in lb names one, branches on one, or
//! ships a seed list of them: `kind` is a closed set of *relationships to the work* (who they are to
//! us), while `name`, `sites`, `trades` and the contact details are workspace strings this crate
//! only ever stores and echoes back. A `trade` is not a vocabulary lb owns either — it is whatever
//! the workspace writes.
//!
//! **Scorecard fields are derived by query, never stored.** "Answered 8 of 11 asks, median 4 h" is a
//! question about the `case_request` rows, and a stored counter is a second writer for a fact the
//! request table already holds — it drifts the first time a request is withdrawn, replayed or
//! deleted. So this record carries no `asks_sent`, no `reply_rate`, no `median_response_h`.
//!
//! State lives in `lb_store` behind the workspace wall; no wall clock in this crate (testing §3).

use serde::{Deserialize, Serialize};

use crate::error::CasesError;

/// The store table parties live in. `kind` and `sites` are `data` fields, so a filtered roster read
/// is a scan with an equality/contains test rather than a second table.
pub const TABLE: &str = "party";

/// What this party is **to the work** — not who they are. A closed set because each value changes
/// what may be asked of them, and an open string here would make `case.request.send` unable to
/// state anything about who it is writing to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartyKind {
    /// Does the physical work and quotes for it.
    Contractor,
    /// Facilities management — approves, coordinates, holds the budget.
    Fm,
    /// Whose building it is, and who ultimately pays.
    Client,
    /// A facilities-management *system* vendor (the ticketing system on the other end).
    Fms,
}

/// How to reach a party.
///
/// **An email is required** ([`validate_party`]); the phone is optional. A roster row exists to be
/// ASKED, `case.request.send` reaches a party by mailing them, and a row with no address is a trap
/// that springs at the worst possible moment — the first time somebody is trying to get a quote
/// out. Refusing it at upsert puts the error in front of the admin who can fix it, seconds after
/// they made the mistake.
///
/// The rule is uniform across [`PartyKind`] rather than gated on it, because every kind is a party
/// the ask plane can write to (the kind decides who holds the ball, not whether they are
/// reachable). If a phone-only party is ever wanted, that is an SMS `Target` arriving with the
/// thing that would make such a row usable — not a hole opened in advance.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contact {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,
}

/// A durable party record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Party {
    /// Stable id, unique within the workspace. Caller-chosen (this is an admin-curated roster, not
    /// a machine-minted record), so a pack can re-seed the same row idempotently.
    pub id: String,
    pub kind: PartyKind,
    /// The human name. Opaque workspace data.
    pub name: String,
    #[serde(default)]
    pub contact: Contact,
    /// The sites this party covers. Opaque workspace strings — the same values the case's `site`
    /// facet carries, which is what makes "who covers this site" answerable without a join table.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sites: Vec<String>,
    /// What they do (`mechanical`, `electrical`, … — whatever the workspace writes). Opaque.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trades: Vec<String>,
    /// How long this party gets to answer an ask, in hours. `0` ⇒ "no opinion", and the request
    /// falls back to the matching `ServicePolicy.party_window_h`. Per-party because a 24 h window
    /// for the people who answer in an hour and for the ones who answer in a week is one of the two
    /// numbers that makes the nudge ladder either useful or noise.
    #[serde(default)]
    pub default_ask_window_h: u32,
}

/// The largest a party's name may be — a bound, not a policy. Long enough for any real company
/// name, short enough that a roster read stays a roster read.
pub const MAX_PARTY_NAME_BYTES: usize = 200;

/// Validate a party BEFORE any write, so a rejected upsert leaves the roster exactly as it was.
///
/// Deliberately thin: an id and a name, and an email that at least looks like one. Nothing here
/// judges a `trade`, a `site` or a phone number — those are workspace vocabulary (rule 10), and a
/// validator that rejected an unfamiliar trade would be lb deciding what trades exist.
pub fn validate_party(party: &Party) -> Result<(), CasesError> {
    if party.id.trim().is_empty() {
        return Err(CasesError::BadInput("a party needs an id".into()));
    }
    if party.id.contains(':') {
        // The id is a store record id and rides into `party:{id}` — a `:` would split the record
        // key. Same one-segment discipline an asset id carries.
        return Err(CasesError::BadInput(
            "a party id may not contain `:` — it is a single record-id segment".into(),
        ));
    }
    if party.name.trim().is_empty() {
        return Err(CasesError::BadInput("a party needs a name".into()));
    }
    if party.name.len() > MAX_PARTY_NAME_BYTES {
        return Err(CasesError::BadInput(format!(
            "a party name is capped at {MAX_PARTY_NAME_BYTES} bytes"
        )));
    }
    // ABSENCE is the failure this catches, and it is the one that used to pass: a misplaced
    // top-level `email` key left `contact` empty, the upsert returned `{ "id": … }`, and the roster
    // held a contractor nobody could reach. Silence at the door became an error at the ask — the
    // exact shape of a 200 that never persisted.
    let Some(email) = party.contact.email.as_deref() else {
        return Err(CasesError::BadInput(format!(
            "party `{}` has no `contact.email` — a party exists to be asked, and an ask is an \
             email; note the address goes under `contact`, not at the top level",
            party.id
        )));
    };
    // The floor, not an RFC 5322 parser: an address with no `@` can never be mailed, and catching
    // it here means the failure surfaces to the admin editing the roster rather than to the relay
    // at 03:00 (`outbox-delivered-is-not-email-sent.md`'s lesson, upstream).
    if email.trim().is_empty() || !email.contains('@') {
        return Err(CasesError::BadInput(format!(
            "party contact email `{email}` is not an address"
        )));
    }
    Ok(())
}
