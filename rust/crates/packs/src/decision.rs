//! The refusal matrix — the pure decision of what a re-apply does, given the incoming manifest and
//! the prior receipt. Isolated here so the whole matrix is one unit-testable function with no I/O:
//! `pack.apply` reads the receipt, calls [`decide`], and acts.
//!
//! The matrix (all proven downstream, ported verbatim — pack-core-scope §"The proven semantics"):
//!   - no prior receipt                         → Apply       (first apply)
//!   - same version, same manifest checksum     → NoOp        (the idempotent re-apply)
//!   - same version, changed manifest checksum  → Refuse      ("bump the version")
//!   - higher version than the receipt          → Upgrade     (re-drive + additive schema reconcile)
//!   - lower version than the receipt           → Refuse      (always — a downgrade)
//!
//! Plus the sixth row that makes recovery work: same version, same checksum, but a PARTIAL prior
//! receipt (a cap-denied or failed object) → Apply again, without re-running rules.
//!
//! Rules run on FIRST apply only — `Apply` carries `run_rules`, true only when there was no prior
//! receipt, so "second run changes nothing" never depends on a rule's dedup key.

use crate::receipt::Receipt;

/// What a re-apply should do.
#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    /// Apply every object. `run_rules` is true only on the very first apply (no prior receipt) — a
    /// partial-recovery re-apply applies objects but never re-runs rules (their dedup keys must not
    /// decide idempotence).
    Apply { run_rules: bool },
    /// A version BUMP (`version > prior.version`) — an upgrade (pack-upgrade-scope). Re-drives every
    /// object like a re-apply (rules never re-run), preserves the operator's rows (seed-ownership),
    /// and ADDITIVELY reconciles the materialized schema (new tables/columns are added; a destructive
    /// change — a dropped/retyped column — is refused, never silently applied). `from`/`to` are the
    /// version pair for the loud "upgraded pack: vN → vM" listing.
    Upgrade { from: u32, to: u32 },
    /// The manifest matches the receipt exactly — change nothing.
    NoOp,
    /// Refuse with a human reason.
    Refuse(String),
}

/// Decide, given the incoming `version` + `manifest_checksum` and the `prior` receipt (if any).
pub fn decide(version: u32, manifest_checksum: &str, prior: Option<&Receipt>) -> Decision {
    let Some(prior) = prior else {
        // First apply: run the rules once.
        return Decision::Apply { run_rules: true };
    };

    if version < prior.version {
        return Decision::Refuse(format!(
            "manifest version {version} is LOWER than the applied version {} — a downgrade is \
             always refused",
            prior.version
        ));
    }
    if version > prior.version {
        // A version bump is an UPGRADE (pack-upgrade-scope): re-drive every object (rules never
        // re-run), preserve the operator's rows, and additively reconcile the materialized schema.
        // Destructive schema changes are refused at apply time, not here (the decision is pure).
        return Decision::Upgrade {
            from: prior.version,
            to: version,
        };
    }
    // Same version.
    if manifest_checksum == prior.manifest_checksum {
        // Same content — a no-op UNLESS the prior apply was partial (a capability deny or a failure
        // left some object un-applied). The documented recovery is "grant the cap, re-run", and
        // idempotence is exactly what makes that safe — so a partial receipt at the same version
        // must re-Apply (re-trying the denied objects, clobbering the applied ones), never a NoOp
        // that would strand the recovery. Rules do NOT re-run (not the first apply).
        if prior.objects.iter().all(|o| o.outcome == "applied") {
            Decision::NoOp
        } else {
            Decision::Apply { run_rules: false }
        }
    } else {
        Decision::Refuse(
            "manifest files changed but the version was NOT bumped — bump `version` in pack.yaml \
             (same version + changed content is refused so a silent drift can't re-apply)"
                .into(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::receipt::ObjectReceipt;

    fn receipt(version: u32, checksum: &str) -> Receipt {
        Receipt {
            pack: "bas".into(),
            title: "Building Automation".into(),
            version,
            manifest_checksum: checksum.into(),
            applied_ts: 0,
            manifest: None,
            objects: vec![],
        }
    }

    fn obj(outcome: &str) -> ObjectReceipt {
        ObjectReceipt {
            kind: "rule".into(),
            id: "r".into(),
            checksum: "c".into(),
            outcome: outcome.into(),
        }
    }

    #[test]
    fn first_apply_runs_rules() {
        assert_eq!(decide(1, "abc", None), Decision::Apply { run_rules: true });
    }

    #[test]
    fn same_version_same_checksum_is_noop() {
        let prior = receipt(1, "abc");
        assert_eq!(decide(1, "abc", Some(&prior)), Decision::NoOp);
    }

    #[test]
    fn same_version_changed_checksum_refuses() {
        let prior = receipt(1, "abc");
        assert!(matches!(
            decide(1, "xyz", Some(&prior)),
            Decision::Refuse(_)
        ));
    }

    #[test]
    fn higher_version_is_an_upgrade() {
        // pack-upgrade-scope: a version bump is now an Upgrade (re-drive objects, reconcile schema
        // additively, preserve rows), not a refusal. The from/to pair rides for the loud listing.
        let prior = receipt(1, "abc");
        assert!(matches!(
            decide(4, "different-checksum", Some(&prior)),
            Decision::Upgrade { from: 1, to: 4 }
        ));
        // A version bump with the SAME checksum is still an upgrade (the version is the signal).
        assert!(matches!(
            decide(2, "abc", Some(&prior)),
            Decision::Upgrade { from: 1, to: 2 }
        ));
    }

    #[test]
    fn lower_version_refuses_downgrade() {
        let prior = receipt(3, "abc");
        assert!(matches!(
            decide(2, "abc", Some(&prior)),
            Decision::Refuse(_)
        ));
    }

    #[test]
    fn same_content_but_prior_partial_reapplies_without_rerunning_rules() {
        // A denied object in the prior receipt means recovery: re-Apply, but do NOT re-run rules.
        let mut prior = receipt(1, "abc");
        prior.objects = vec![obj("applied"), obj("denied")];
        assert_eq!(
            decide(1, "abc", Some(&prior)),
            Decision::Apply { run_rules: false }
        );
    }

    #[test]
    fn same_content_fully_applied_is_noop() {
        let mut prior = receipt(1, "abc");
        prior.objects = vec![obj("applied"), obj("applied")];
        assert_eq!(decide(1, "abc", Some(&prior)), Decision::NoOp);
    }

    #[test]
    fn reapply_never_reruns_rules() {
        // The only Apply that runs rules is the first (no prior receipt); a re-apply that reaches
        // Apply cannot exist for the same version, but the invariant is: run_rules ⟺ no prior.
        assert_eq!(decide(1, "abc", None), Decision::Apply { run_rules: true });
    }
}
