//! The retention lint — the one thing standing between a pack author's typo and a policy that
//! applies with the field silently dropped.
//!
//! `manifest_retention` holds `method` and `range.mode` as `String` (it is a dependency-free mirror
//! of the verb args — see that module for why), so nothing in the type system rejects `method: p95`.
//! The apply-side conversion cannot reject it either without failing the whole pack opaquely. So it
//! is caught HERE, at validate time, where the author is still looking and the message can name the
//! closed set. Errors gate the apply (`validate.rs` module doc).

use crate::manifest::RetentionPolicy;
use crate::validate::Finding;

/// The closed set of rollup-tier methods, mirroring `lb_ingest::Method`.
pub const METHODS: [&str; 8] = [
    "avg", "min", "max", "sum", "count", "last", "first", "nearest",
];

/// Lint every retention policy's `method` / `range.mode` names.
pub fn lint(policies: &[RetentionPolicy]) -> Vec<Finding> {
    let mut out = Vec::new();
    // ERROR — an unknown retention tier `method` or range `mode`. The manifest is a dependency-free
    // MIRROR of the verb args (both are held as `String`), so this lint is the only thing standing
    // between an author's typo and a policy that applies with the field silently dropped — the
    // closed-struct trap. Named here, at validate time, where the author is still looking.
    for policy in policies {
        for tier in &policy.tiers {
            if let Some(m) = &tier.method {
                if !METHODS.contains(&m.as_str()) {
                    out.push(Finding {
                        error: true,
                        message: format!(
                        "retention '{}': tier {}ms has unknown method '{m}' — expected one of {}",
                        policy.prefix,
                        tier.width_ms,
                        METHODS.join(", ")
                    ),
                    });
                }
            }
        }
        if let Some(mode) = policy
            .filter
            .as_ref()
            .and_then(|f| f.range.as_ref())
            .and_then(|r| r.mode.as_deref())
        {
            if mode != "drop" && mode != "clamp" {
                out.push(Finding {
                error: true,
                message: format!(
                    "retention '{}': filter.range has unknown mode '{mode}' — expected drop or clamp",
                    policy.prefix
                ),
            });
            }
        }
    }
    out
}
