//! The pure capture-outcome decision: given the static plan, the before-image read outcome, and
//! the runtime taint, decide what (if anything) to journal. Extracted as a pure function (no store,
//! no I/O) so the one load-bearing distinction can never silently regress: a **failed** before-image
//! read is NOT the same as a read that found the record **absent**.
//!
//! The bug this encodes against (debugging/undo/read-error-journals-absent-before-image.md): the
//! capture path used to map a `read_versioned` *error* to the same shape as a genuine absence
//! (`before: None, rev 0`), so a transient read failure on an EXISTING record journaled a tombstone
//! before-image — and a later undo would have deleted real data (the rev predicate guards the
//! *after* state, so it passes). The rule is: only a successful read that finds nothing is
//! `absent`; a read **error** makes the step **not-undoable** — undo refuses it instead of
//! restoring a before-image nobody actually observed.

use lb_store::Versioned;
use serde_json::Value;

use super::plan::CapturePlan;

/// The before-image read outcome for a reversible-planned call. `Failed` is a read *error* —
/// deliberately distinct from `Read(Versioned::absent())`, which is a successful read of a
/// genuinely absent record (a create).
pub(crate) enum BeforeRead {
    Read(Versioned),
    Failed,
}

/// What the capture seam should journal for a completed, successful tool call.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Decision {
    /// The transaction reached the outbox: journal not-undoable, classified from the taint
    /// (irreversible, or compensable if a compensation is declared). Taint wins over everything.
    Tainted,
    /// A capturable reversible step: journal an undoable before-image entry.
    Undoable {
        before: Option<Value>,
        before_rev: u64,
    },
    /// Journal a not-undoable marker: the call mutated durable state but no safe before-image
    /// exists (a non-generic mutation, or a reversible plan whose before-image read FAILED).
    NotUndoable,
    /// Nothing to journal (a pure read).
    Nothing,
}

/// Map (plan, before-read, taint) → what to journal. Pure — the whole outcome table in one place.
pub(crate) fn decide(
    plan: &CapturePlan,
    before: Option<BeforeRead>,
    reached_outbox: bool,
    wrote_store: bool,
) -> Decision {
    // Taint wins over the static plan (the max-composition rule, derived from what actually ran).
    if reached_outbox {
        return Decision::Tainted;
    }
    match plan {
        CapturePlan::Reversible { .. } => match before {
            // A successful read — present or genuinely absent — is a trustworthy before-image.
            Some(BeforeRead::Read(v)) => Decision::Undoable {
                before: v.value,
                before_rev: v.rev,
            },
            // A read ERROR is not absence: nobody observed the prior state, so restoring
            // "absent" could delete real data. The step is journaled not-undoable instead.
            Some(BeforeRead::Failed) | None => Decision::NotUndoable,
        },
        // Mutated the store but the touched set is unknowable → not-undoable marker.
        CapturePlan::NonGeneric if wrote_store => Decision::NotUndoable,
        CapturePlan::NonGeneric | CapturePlan::NotMutating => Decision::Nothing,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn reversible() -> CapturePlan {
        CapturePlan::Reversible {
            table: "inbox".into(),
            id: "general__m1".into(),
        }
    }

    /// THE regression: a failed before-image read must mark the step not-undoable — never journal
    /// "absent" (which a later undo would restore by DELETING the real record).
    #[test]
    fn read_error_is_not_undoable_never_absent() {
        let d = decide(&reversible(), Some(BeforeRead::Failed), false, true);
        assert_eq!(d, Decision::NotUndoable);
        assert_ne!(
            d,
            Decision::Undoable {
                before: None,
                before_rev: 0
            },
            "a read error must never be journaled as a create tombstone"
        );
    }

    /// Only a SUCCESSFUL read that finds nothing is `absent` (a genuine create — undoable).
    #[test]
    fn successful_absent_read_is_an_undoable_create() {
        let d = decide(
            &reversible(),
            Some(BeforeRead::Read(Versioned::absent())),
            false,
            true,
        );
        assert_eq!(
            d,
            Decision::Undoable {
                before: None,
                before_rev: Versioned::ABSENT_REV
            }
        );
    }

    #[test]
    fn successful_present_read_carries_the_before_image() {
        let v = Versioned {
            value: Some(json!({"body": "prior"})),
            rev: 7,
        };
        let d = decide(&reversible(), Some(BeforeRead::Read(v)), false, true);
        assert_eq!(
            d,
            Decision::Undoable {
                before: Some(json!({"body": "prior"})),
                before_rev: 7
            }
        );
    }

    /// Taint wins over everything — even a clean before-image (the max-composition rule).
    #[test]
    fn outbox_taint_wins_over_a_clean_before_image() {
        let v = Versioned {
            value: Some(json!({})),
            rev: 1,
        };
        assert_eq!(
            decide(&reversible(), Some(BeforeRead::Read(v)), true, true),
            Decision::Tainted
        );
        // ...and over a failed read too (still classified from the taint, not the read).
        assert_eq!(
            decide(&reversible(), Some(BeforeRead::Failed), true, true),
            Decision::Tainted
        );
    }

    /// A missing before snapshot for a reversible plan is defensively not-undoable (never absent).
    #[test]
    fn missing_snapshot_is_not_undoable() {
        assert_eq!(
            decide(&reversible(), None, false, true),
            Decision::NotUndoable
        );
    }

    #[test]
    fn non_generic_mutation_is_not_undoable_and_pure_read_is_nothing() {
        assert_eq!(
            decide(&CapturePlan::NonGeneric, None, false, true),
            Decision::NotUndoable
        );
        assert_eq!(
            decide(&CapturePlan::NonGeneric, None, false, false),
            Decision::Nothing
        );
        assert_eq!(
            decide(&CapturePlan::NotMutating, None, false, false),
            Decision::Nothing
        );
    }
}
