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

use crate::compact::CompactionRecord;

/// Skip the boot compaction pass when the commit log is larger than this fraction of available RAM.
///
/// **What the pass actually costs (measured 2026-08-01 — see the session doc).** Peak RSS over log
/// bytes is *record-size dependent*, because SurrealKV's boot memory tracks the index (keys +
/// offsets), not the values:
///   - a fat-record store (220k keys x 3.9 KB, 1.34 GB log): the pass peaked at **0.26x** the log
///     (0.41x the live set); the same open with the pass skipped peaked at 0.11x;
///   - the incident box's key-dense store (617 MB log of ~700-byte ingest samples): **~1.4x** the
///     log (879 MB RSS on a 959 MB machine).
///
/// So 0.5 is ~2x loose for fat records and still tighter than the key-dense case needs — which is
/// the right asymmetry: declining a pass that would have fit costs a slower boot on an uncompacted
/// log; running one that does not fit costs the whole machine.
pub const BOOT_COMPACT_MEM_RATIO: f64 = 0.5;

/// Refuse to open at all when the commit log is larger than this fraction of available RAM. The
/// plain replay still builds the whole live-set index in RAM — skipping compaction lowers the peak
/// but does not cap it.
///
/// Deliberately looser than [`BOOT_COMPACT_MEM_RATIO`]: refusing a boot is a far bigger call than
/// skipping a pass, and a plain replay peaks well below a merge (measured at 0.11x the log on the
/// fat-record 1.34 GB store above; far higher on a key-dense one). At 1.0 the guard fires only for a
/// store that provably cannot fit, converting a machine-wide OOM into a millisecond-cheap, legible
/// refusal — recoverable in one ssh session (see [`crate::StoreError::WontFit`]).
pub const OPEN_GUARD_MEM_RATIO: f64 = 1.0;

/// How much the log must have grown since an unproductive pass before it is worth re-trying one.
/// A quarter of fresh bloat is enough reclaimable material to justify the attempt; below it the
/// skip stands, because re-compacting a log that *is* the live set is the most expensive possible
/// no-op (scope decision 2).
pub const REGROWTH_RERUN_RATIO: f64 = 1.25;

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

/// Why boot declined to run the compaction pass — the string carried in the record's `skipped`
/// field and logged at warn. `None` ⇒ run the pass (today's behaviour).
///
/// - `available_ram` is `None` on a machine we cannot measure ⇒ the headroom precondition passes
///   (fail open, scope decision 5).
/// - `last` is the **persisted** record of the previous real pass (`last-compaction.json`), or
///   `None` when there is none / it was unreadable ⇒ the benefit precondition passes.
pub fn boot_compaction_skip(
    log_bytes: u64,
    available_ram: Option<u64>,
    last: Option<&CompactionRecord>,
) -> Option<String> {
    if let Some(avail) = available_ram {
        let allowance = BOOT_COMPACT_MEM_RATIO * avail as f64;
        if (log_bytes as f64) > allowance {
            return Some(format!(
                "log {log_bytes} bytes exceeds {pct:.0}% of available RAM ({avail} bytes) — the \
                 boot compaction pass can peak at more than the log size (measured 0.26x on a \
                 fat-record store, ~1.4x on a key-dense one) and could OOM this machine; \
                 skipping it and opening on the uncompacted log",
                pct = BOOT_COMPACT_MEM_RATIO * 100.0,
            ));
        }
    }
    let last = last?;
    // A failed or itself-skipped pass says nothing about whether compaction pays here.
    if !last.ok || last.skipped.is_some() {
        return None;
    }
    if is_productive(last.before_bytes, last.after_bytes) {
        return None;
    }
    if (log_bytes as f64) > REGROWTH_RERUN_RATIO * last.after_bytes as f64 {
        return None; // grown materially since — fresh bloat is worth another pass
    }
    Some(format!(
        "the last pass reclaimed almost nothing ({before} → {after} bytes) and the log has not \
         grown past {ratio}x that since (now {log_bytes} bytes) — this log is the live set; \
         skipping a boot pass that would cost peak memory for nothing",
        before = last.before_bytes,
        after = last.after_bytes,
        ratio = REGROWTH_RERUN_RATIO,
    ))
}

/// True when this machine provably cannot replay this log — the open must be refused rather than
/// attempted. `available_ram` of `None` ⇒ false (fail open).
pub fn open_would_not_fit(log_bytes: u64, available_ram: Option<u64>) -> bool {
    match available_ram {
        Some(avail) => (log_bytes as f64) > OPEN_GUARD_MEM_RATIO * avail as f64,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(before: u64, after: u64) -> CompactionRecord {
        CompactionRecord {
            at_epoch_ms: 1,
            ok: true,
            before_bytes: before,
            after_bytes: after,
            duration_ms: 1,
            error: None,
            skipped: None,
        }
    }

    #[test]
    fn headroom_precondition() {
        // 617 MB log, 802 MB available — the incident box. Over half ⇒ skip.
        let log = 617 * 1024 * 1024;
        let avail = 802 * 1024 * 1024;
        let reason = boot_compaction_skip(log, Some(avail), None).expect("skips");
        assert!(reason.contains(&log.to_string()) && reason.contains(&avail.to_string()));
        // Same log on a box with 4 GB free: comfortably under half ⇒ run.
        assert_eq!(boot_compaction_skip(log, Some(4 << 30), None), None);
        // Exactly at the ratio is allowed (strictly-greater is the skip).
        assert_eq!(boot_compaction_skip(100, Some(200), None), None);
        assert!(boot_compaction_skip(101, Some(200), None).is_some());
    }

    #[test]
    fn unmeasurable_ram_fails_open() {
        assert_eq!(boot_compaction_skip(u64::MAX, None, None), None);
        assert!(!open_would_not_fit(u64::MAX, None));
    }

    #[test]
    fn unproductive_last_pass_precondition() {
        // Reclaimed 0.3%: unproductive. Log unchanged ⇒ skip.
        let last = rec(617_000_000, 615_000_000);
        assert!(boot_compaction_skip(615_000_000, Some(u64::MAX), Some(&last)).is_some());
        // Grown past 1.25x ⇒ run again.
        assert_eq!(
            boot_compaction_skip(
                (615_000_000.0 * REGROWTH_RERUN_RATIO) as u64 + 1,
                Some(u64::MAX),
                Some(&last)
            ),
            None
        );
        // A productive last pass never suspends the next one.
        let paid = rec(1_000_000, 20_000);
        assert_eq!(
            boot_compaction_skip(1_000_000, Some(u64::MAX), Some(&paid)),
            None
        );
        // A failed or skipped record concludes nothing.
        let mut failed = rec(617_000_000, 615_000_000);
        failed.ok = false;
        assert_eq!(
            boot_compaction_skip(615_000_000, Some(u64::MAX), Some(&failed)),
            None
        );
        let mut skipped = rec(617_000_000, 615_000_000);
        skipped.skipped = Some("earlier skip".into());
        assert_eq!(
            boot_compaction_skip(615_000_000, Some(u64::MAX), Some(&skipped)),
            None
        );
    }

    #[test]
    fn productivity_judgement_matches_the_runtime_driver() {
        assert!(is_productive(0, 0));
        assert!(is_productive(1000, 900));
        assert!(!is_productive(1000, 901));
    }

    #[test]
    fn open_guard_ratio() {
        // The incident box: 617 MB log, 802 MB available — allowed (this is the expected outcome).
        assert!(!open_would_not_fit(617 << 20, Some(802 << 20)));
        // A 900 MB log on the same box: refused.
        assert!(open_would_not_fit(900 << 20, Some(802 << 20)));
        // Exactly at the ratio fits.
        assert!(!open_would_not_fit(802 << 20, Some(802 << 20)));
    }
}
