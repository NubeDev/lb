//! The **store** boot-config env readers — `LB_STORE_MAX_BYTES` (the disk budget, issue #122) and
//! `LB_STORE_OPEN_UNGUARDED` (the boot memory-guard override, issue #128).
//!
//! Split out of `config.rs` rather than added to it: that file is long past the FILE-LAYOUT limit,
//! and "the two knobs that decide how much of this machine the store may take" is a real, named
//! responsibility — not a bag of parsers. Both are read at the **binary boundary**; no library code
//! below the boot seam reads env (`BootConfig` carries the values down).
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

/// Parse `LB_STORE_OPEN_UNGUARDED` into the boot memory-guard override (boot-memory-guard scope,
/// decision 6). Only the exact value `1` disables the guard; unset/empty ⇒ the guard stays on
/// silently, and any OTHER value warns and leaves it ON — a typo must never quietly remove the
/// protection that keeps a box reachable. It disables the *open* guard only: the boot-compaction
/// preconditions are not overridable, because skipping a pass is always safe.
pub(crate) fn store_open_unguarded_from_env() -> bool {
    match std::env::var("LB_STORE_OPEN_UNGUARDED") {
        Ok(v) if v.trim() == "1" => {
            eprintln!(
                "LB_STORE_OPEN_UNGUARDED=1: the store boot memory guard is DISABLED — a commit log \
                 larger than available RAM will be opened anyway, which can OOM this machine"
            );
            true
        }
        Ok(v) if !v.trim().is_empty() => {
            eprintln!(
                "bad LB_STORE_OPEN_UNGUARDED '{v}': only the exact value '1' disables the store \
                 boot memory guard — leaving it ON"
            );
            false
        }
        _ => false,
    }
}
