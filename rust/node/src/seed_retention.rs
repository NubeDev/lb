//! **Provision retention policies at boot** — `BootConfig::retention_seed`, applied before anything
//! can ingest.
//!
//! ## Why this is a boot step and not an operator action
//!
//! A retention policy is a row in the store. A rebuilt, reflashed or wiped instance therefore starts
//! with **no** policy — which means keep-everything-for-ever, the loosest setting there is. That is
//! the wrong default in the one window where it matters most: on a fresh node raw samples are not yet
//! old enough for any time horizon to evict, so for the whole cold-start period nothing bounds
//! growth, and compaction alone cannot bound a pure-append stream.
//!
//! rubix-ai#84 is the live case. A node had bounds applied by hand and proven working; it was rebuilt
//! four days later and came back with the stock keep-for-ever policy, silently, and grew ~1.09 GB of
//! commit log in ~2.5 h until the #128 boot guard refused to open the store. Every guard that
//! survived the rebuild was one held in DECLARED STATE (the kernel cmdline on the image, the
//! generated systemd unit). Every guard that vanished was one held inside the instance. So the fix is
//! not a better runbook — it is to move the policy into declared boot config, which is what this
//! module is.
//!
//! ## Two rules it must not break
//!
//! 1. **Never stomp an operator's policy.** Seeding is strictly *only-if-absent*, per prefix. A
//!    policy an operator set — including a deliberate keep-for-ever one — survives every restart
//!    untouched. This is what rubix-ai#84's AC 8/9 checked, and it is why this cannot be an UPSERT.
//! 2. **`0` still means unbounded.** No default shape changes and no existing row is rewritten;
//!    absent fields keep deserializing to `0`. A seed only ever fills a prefix that had no row at
//!    all.
//!
//! ## Ordering — the load-bearing part
//!
//! This runs from [`crate::seeds::run`], which the boot ritual calls **before** the gateway/roles
//! block spawns native sidecars. That ordering is the whole point: an extension that stands its own
//! only-if-absent policy for its prefix (the normal pattern) starts after this, finds the prefix
//! already occupied, and correctly declines to overwrite. Seeding after `boot_full` returned would be
//! a race against sidecar startup, and on a slow edge box the failsafe would be the one that loses.
//!
//! ## Naming nothing
//!
//! The prefixes and horizons are entirely config — this module knows no extension, no product and no
//! series name (rule 10). A host that seeds nothing passes an empty vec and boot is byte-identical to
//! before.

use std::sync::Arc;

use lb_host::Node;
use lb_ingest::{list_policies, set_policy};

use crate::config::BootConfig;

/// The `updated_by` provenance stamped on a seeded row.
///
/// Deliberately distinguishable from both a human and an extension: the first question asked of
/// rubix-ai#84's node was "did an operator set this, or did something re-provision it?", and neither
/// the row nor any log could answer. A seeded policy now says so on its face, and an operator who
/// edits it takes over the field.
pub const SEEDED_BY: &str = "node:boot-seed";

/// Stand every policy in `cfg.retention_seed` that has no row yet, for `cfg.workspace`.
///
/// Best-effort and non-fatal, matching every other boot seeder: a store that cannot be read logs and
/// leaves the node serving. It is loud on the way past, though — a seed that could not be applied is
/// exactly the silent reversion this exists to prevent.
pub async fn run(node: &Arc<Node>, cfg: &BootConfig) {
    if cfg.retention_seed.is_empty() {
        return;
    }
    let ws = &cfg.workspace;

    // ONE read for the whole pass — the only-if-absent test every seed below makes. Read first and
    // compare, rather than probing per prefix, so N seeds cost one query.
    let existing: Vec<String> = match list_policies(&node.store, ws).await {
        Ok(ps) => ps.into_iter().map(|p| p.prefix).collect(),
        Err(e) => {
            eprintln!(
                "boot: retention seed for ws={ws} SKIPPED — could not read the existing policies \
                 ({e}). This node may be running with NO retention bound; check \
                 `series.retention.list`."
            );
            return;
        }
    };

    for seed in &cfg.retention_seed {
        if existing.iter().any(|p| p == &seed.prefix) {
            // Not a warning: this is the steady state on every boot after the first, and on every
            // node whose operator has set their own policy. Both are correct outcomes.
            println!(
                "boot: retention seed '{}' already present for ws={ws} — left untouched",
                seed.prefix
            );
            continue;
        }
        let mut policy = seed.clone();
        policy.updated_by = Some(SEEDED_BY.to_string());
        policy.updated_ms = Some(now_ms());
        match set_policy(&node.store, ws, &policy).await {
            Ok(()) => println!(
                "boot: retention seed '{}' provisioned for ws={ws} (raw_for_ms={} max_samples={} \
                 tiers={})",
                policy.prefix,
                policy.raw_for_ms,
                policy.max_samples,
                policy.tiers.len()
            ),
            Err(e) => eprintln!(
                "boot: retention seed '{}' for ws={ws} FAILED ({e}) — series under this prefix are \
                 unbounded until a policy is set.",
                policy.prefix
            ),
        }
    }
}

/// Wall-clock ms for the provenance stamp. Only ever a `updated_ms` label — nothing in the seed
/// decision reads the clock, which is deliberate on a box whose clock resets every power cycle.
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
