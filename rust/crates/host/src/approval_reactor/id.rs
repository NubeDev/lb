//! The one place the held-effect id is derived from its approval item id (rules-approvals scope).
//!
//! A rule's `inbox.request_approval` stages the gated effect under this id and records the
//! `needs:approval` item under `item_id`; the reactor, seeing an `Approved`/`Rejected` resolution for
//! `item_id`, reconstructs the effect id with the SAME function to release or discard it. Deriving the
//! id (rather than a sidecar table) is what keeps the reactor domain-free (rule 10): it keys on opaque
//! ids, never on any extension or effect semantics.

/// The outbox effect id that gates on approval item `item_id`. Stable and deterministic (no
/// wall-clock/random), so a re-run of the same rule upserts the same held effect and the reactor
/// always addresses it.
pub fn held_effect_id(item_id: &str) -> String {
    format!("held:{item_id}")
}
