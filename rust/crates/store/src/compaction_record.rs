//! [`CompactionRecord`] — what one compaction pass (boot or online) cost, and where that cost went.
//!
//! Its own file because it is the pass's *published result*, not part of running one: it is returned
//! by the `store.compact` job, served by `store.status`, and persisted beside the store for the next
//! boot to read (`last_pass`). Three readers, none of which needs the compaction engine.

use serde::{Deserialize, Serialize};

/// Outcome of one compaction pass (boot or online). Served by `status`, returned by the
/// `store.compact` job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionRecord {
    /// Wall-clock epoch ms when the pass finished.
    pub at_epoch_ms: u64,
    pub ok: bool,
    /// Commit-log bytes before / after the pass (after includes the applied merge).
    pub before_bytes: u64,
    pub after_bytes: u64,
    /// How long the blocking pass took.
    pub duration_ms: u64,
    /// The failure, when `ok` is false. A failed pass leaves the log exactly as it was.
    pub error: Option<String>,
    /// Set when the pass did not run because a **boot precondition** declined it — the machine
    /// lacked the memory headroom, or the last pass reclaimed essentially nothing and the log has
    /// not grown since (boot-memory-guard scope slice 1). Carries the human-readable reason, which
    /// is the same string logged at warn. `ok` is false for a skip (nothing was compacted) but
    /// `error` is `None` — a skip is a decision, not a failure, and every reader that judges
    /// whether compaction still pays must ignore it rather than conclude from it.
    ///
    /// Defaulted on deserialize so a record persisted by an older node still loads.
    #[serde(default)]
    pub skipped: Option<String>,
    /// Where `duration_ms` actually went. Every field defaulted, so an older persisted record loads.
    #[serde(default)]
    pub phases: CompactionPhases,
}

/// The four phases of a pass, in the order they run — the split that says WHICH one to attack.
///
/// Both plausible fixes for the ~94 s pause measured on RC-6 target a different phase, and the
/// phases are not separable a priori: [`CompactionPhases::open_ms`] is one sequential replay of the
/// whole log to rebuild the in-memory index, and [`CompactionPhases::compact_ms`] is a scattered
/// `pread`-per-value pass over the live subset. Which dominates depends on how much of a given
/// node's log is live, so it is a property of the workload, not of the code — and therefore a thing
/// to MEASURE on the node in front of you rather than reason about. This struct is that measurement,
/// carried on the record the `store.compact` job returns and the one persisted beside the store, so
/// the answer survives to the next boot.
///
/// The two remaining phases are expected to be small (they replay the already-compacted log) and are
/// recorded precisely so that "expected" can be checked instead of assumed.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct CompactionPhases {
    /// Waiting for the dropped engine to quiesce before the pass may start. Online pass only —
    /// zero at boot, where nothing else holds the directory. This time is spent under the write
    /// guard, so it counts against write availability even though no compaction work happens in it.
    #[serde(default)]
    pub quiesce_ms: u64,
    /// `surrealkv::Store::new` — full sequential replay of the UNCOMPACTED log.
    #[serde(default)]
    pub open_ms: u64,
    /// `surrealkv::Store::compact()` + close — the index-snapshot walk, the per-value reads, and
    /// writing the live set out to `.merge/`.
    #[serde(default)]
    pub compact_ms: u64,
    /// The throwaway open that applies the pending `.merge/` — the physical swap plus a replay of
    /// the now-compacted log.
    #[serde(default)]
    pub merge_ms: u64,
}
