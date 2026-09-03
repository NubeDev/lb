//! The **store** boot-config env readers — `LB_STORE_MAX_BYTES` (the disk budget, issue #122),
//! `LB_RETENTION_PERIOD_SECS` (the retention-GC cadence, rubix-ai#84).
//!
//! Split out of `config.rs` rather than added to it: that file is long past the FILE-LAYOUT limit,
//! and "the knobs that decide how much of this machine the store may take, and how often it is
//! trimmed" is a real, named responsibility — not a bag of parsers. All are read at the **binary
//! boundary**; no library code below the boot seam reads env (`BootConfig` carries the values down).
//!
//! Shared posture, from `LB_MAX_EXTENSION_UPLOAD_BYTES`: a malformed value **warns and falls back**
//! to the safe default. Boot never panics over a typo, and a typo never silently removes a guard.

/// Parse `LB_STORE_MAX_BYTES` (a plain byte count) into the node's store disk budget (disk-budget
/// scope, slice 1); unset/empty/unparseable ⇒ `None` ⇒ today's exact behaviour (the flat
/// [`lb_host::LOG_ADVISORY_BYTES`] advisory, no marks — scope decisions 1 and 2).
pub(crate) fn store_budget_bytes_from_env() -> Option<u64> {
    match std::env::var("LB_STORE_MAX_BYTES") {
        Ok(v) if !v.trim().is_empty() => v.trim().parse().ok().or_else(|| {
            eprintln!(
                "bad LB_STORE_MAX_BYTES '{v}': not a byte count — running with no store disk budget"
            );
            None
        }),
        _ => None,
    }
}

/// Parse `LB_RETENTION_PERIOD_SECS` into the retention-GC cadence; unset/empty/unparseable/`0` ⇒
/// `None` ⇒ [`lb_host::RETENTION_PERIOD`] (300 s), today's exact behaviour.
///
/// **This exists to make the cadence TESTABLE, not to make it fast.** The 300 s default stays
/// (`retention_reactor.rs`): a pass is a full table scan — a `count()` per series behind the store's
/// global session mutex, up to 10k series per workspace — and `debugging/agent/dev-node-cpu-job-scan.md`
/// is the precedent for why a fast tick over a full table scan is a CPU bug waiting to happen.
/// Before this, verifying the reactor's cadence on a dev box meant EDITING the const and rebuilding
/// lb, which is why every retention proof to date drove `series.retention.gc` by hand instead of
/// observing the reactor. Lowering this in production is an operator's explicit choice, taken with
/// the cost above in view.
///
/// `0` is folded into the default deliberately: `Duration::ZERO` would spin `tokio::time::interval`
/// into a hot loop, so the one value that could hang a node is the one value that cannot be set.
pub(crate) fn retention_period_from_env() -> Option<std::time::Duration> {
    match std::env::var("LB_RETENTION_PERIOD_SECS") {
        Ok(v) if !v.trim().is_empty() => match v.trim().parse::<u64>() {
            Ok(0) => {
                eprintln!(
                    "LB_RETENTION_PERIOD_SECS=0 would spin the retention tick — using the default"
                );
                None
            }
            Ok(secs) => Some(std::time::Duration::from_secs(secs)),
            Err(_) => {
                eprintln!(
                    "bad LB_RETENTION_PERIOD_SECS '{v}': not a whole number of seconds — using \
                     the default retention cadence"
                );
                None
            }
        },
        _ => None,
    }
}
