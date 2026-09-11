//! The boot memory guards — **pure arithmetic** over three numbers (log bytes, available RAM, the
//! last pass's outcome) that decides whether boot may run a compaction pass and whether the store
//! may be opened at all (boot-memory-guard scope, issue #128).
//!
//! Why this file exists: `Store::open` used to run a full compaction pass unconditionally and then
//! replay the log again, with nothing on the path knowing how much RAM the machine had. On a
//! 959 MB box with a 617 MB live set that drove 879 MB anon-RSS, the kernel's **global** OOM killer
//! took `sshd` down with the node, and `Restart=on-failure` re-ran it every 5 s until someone drove
//! to the site. A node that cannot open its store needs an operator, never the OOM killer.
//!
//! Everything here is a pure function so the gigabyte-scale judgements are tested by *injecting the
//! numbers* — no seeding 617 MB in CI, and no mock either: the callers feed these functions real
//! measured bytes (rule 9).

/// A pass that leaves more than this fraction of the log behind reclaimed essentially nothing.
/// Lives here (rather than in the host's budget driver, which re-exports it) so the boot
/// precondition and the runtime driver make the **same** judgement from one definition.
pub const PRODUCTIVE_RECLAIM_RATIO: f64 = 0.9;

/// Did the pass reclaim enough to be worth the write pause? A pass over an empty log is productive
/// by convention (there was nothing to reclaim, so nothing to conclude).
pub fn is_productive(before_bytes: u64, after_bytes: u64) -> bool {
    if before_bytes == 0 {
        return true;
    }
    (after_bytes as f64) <= PRODUCTIVE_RECLAIM_RATIO * (before_bytes as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn productivity_judgement_matches_the_runtime_driver() {
        assert!(is_productive(0, 0));
        assert!(is_productive(1000, 900));
        assert!(!is_productive(1000, 901));
    }
}
