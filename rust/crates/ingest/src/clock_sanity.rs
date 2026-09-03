//! **Is the node's clock plausible?** — the check that makes an inert GC pass distinguishable from
//! a healthy idle one (issue NubeIO/rubix-ai#84, AC 7).
//!
//! # The failure this exists for
//!
//! Every bound in this subsystem is computed as `now_ms - horizon`. `now_ms` is caller-injected
//! (determinism §3), and on the field hardware it comes from a wall clock that can be badly wrong:
//! the RC-6 unit has no RTC battery and its bench LAN has no NTP route, so it boots at whatever the
//! filesystem timestamp seeds and drifts from there. Observed live 2026-08-04: a 46-minute skew put
//! `now_ms` BEHIND every sample on disc, so every horizon landed before the oldest row, and
//! `series.retention.gc` returned `evicted_raw: 0, warnings: 0`.
//!
//! That return value is **byte-identical to a healthy pass on a node with nothing to evict.** After
//! the clock was corrected the same call evicted 702 raw samples. Nothing on the box could tell the
//! two apart, and the clock resets on every power cycle — a green status that means "dead" is the
//! exact failure class this subsystem cannot afford, because the store then grows unbounded while
//! every observable says retention is working.
//!
//! # The signal
//!
//! The store itself carries a lower bound on the true time: **data cannot exist in the future.**
//! Every committed sample's `ts` is a moment that has already happened, so the newest `ts` in the
//! workspace is a timestamp the real clock has already passed. If `now_ms` is meaningfully BEHIND
//! that, one of two things is true and both are worth saying out loud:
//!
//!   - the node's clock is behind (the observed case — the GC is inert), or
//!   - a producer is stamping timestamps from its own skewed clock (a data problem, and the reason
//!     the message names the series rather than only the delta).
//!
//! Either way the horizons are being computed against a `now_ms` the data disagrees with, and that
//! is worth a warning whichever half is at fault. The check deliberately does NOT try to attribute
//! blame or to correct anything: it reports a contradiction between two clocks, which is a fact,
//! and leaves the operator to decide which one is wrong.
//!
//! # What this check CANNOT see — state it plainly rather than let it be discovered
//!
//! This compares two clocks. It is therefore blind, by construction, whenever the two clocks are
//! **wrong together**, and that is not a hypothetical: the modbus sidecar stamps every sample's `ts`
//! from `SystemTime::now()` on the same box that runs the node. On an RC with no RTC, the producer
//! and the retention pass share one bad clock, the data agrees with `now_ms` perfectly, and nothing
//! here fires — while every horizon is still computed against a time that never happened.
//!
//! Two consequences worth being precise about:
//!
//!   - **Relative bounds still hold.** With one shared clock the horizons are self-consistent: "keep
//!     30 minutes of raw" keeps 30 minutes of *that clock's* minutes, which is the right amount of
//!     data. A uniformly-shifted clock is mostly a labelling problem, not a retention one.
//!   - **A clock JUMP is the real hazard.** When the shared clock corrects (NTP arrives) or resets
//!     (power cycle), `now_ms` moves discontinuously relative to data stamped before the jump — and
//!     *that* this check does see, because the pre-jump data and the post-jump clock now disagree.
//!
//! So the coverage is: a clock that disagrees with the data on disc, in either direction. What is
//! NOT covered is a node that has never had a correct clock and has no data predating the error —
//! most sharply, a **fresh boot with an empty series table**, where there is no evidence at all to
//! contradict `now_ms`. Closing that needs an INDEPENDENT reference (a persisted last-known-good
//! time, or NTP) checked at boot, which is a different mechanism at a different layer and not
//! something `run_gc` can do (rubix-ai#84 AC 8).
//!
//! # Why a warning and not an error
//!
//! Refusing to run the GC on a skewed clock would turn a degraded node into a stopped one, and a
//! stopped GC is the very thing being guarded against. The pass runs exactly as before; it just
//! stops being silent. The warning rides `GcPass::warnings` — the channel that already exists for
//! "an eviction decision the operator should see" — so it reaches the `series.retention.gc` caller,
//! the reactor's log, AND the persisted `series_gc_pass` row without any new plumbing.
//!
//! # Why the tolerance is what it is
//!
//! [`SKEW_TOLERANCE_MS`] is 5 minutes. The floor is set by legitimate producer skew: a modbus
//! gateway stamping from its own clock is routinely tens of seconds out, and a warning that fires on
//! that is noise that gets ignored — which would put us back where we started. The ceiling is set by
//! the horizons this subsystem actually runs: the shipped modbus default keeps raw for 30 minutes on
//! 15-minute buckets, so a skew of 5 minutes is already a third of a bucket and heading for
//! trouble. It is a `pub const` rather than policy config on purpose — a per-policy knob here is an
//! invitation to tune the alarm until it stops firing.

use lb_store::{Store, StoreError};

use crate::schema::SERIES_LATEST_TABLE;

/// How far ahead of `now_ms` the newest sample may sit before the pass warns.
///
/// See the module docs for why 5 minutes and why it is not configurable.
pub const SKEW_TOLERANCE_MS: u64 = 5 * 60 * 1_000;

/// **Has this node's clock gone BACKWARDS since its last recorded GC pass?**
///
/// The second, independent signal — and the one that survives the blind spot in the module docs. It
/// does not compare the clock to the data (which a shared bad clock defeats); it compares the clock
/// to *this node's own past*, which is a monotonic floor: a pass ran at `last_run_ms`, so the true
/// time is at least that. A `now_ms` meaningfully below it means the clock moved backwards between
/// two boots, which a correct clock cannot do.
///
/// This is what covers the **power cycle on a box with no RTC** (rubix-ai#84 AC 8): the store may
/// hold no samples at all — the case [`skew`] is structurally blind to — but if this node has ever
/// run a GC pass, it has a floor to check against, and that floor is written on every pass including
/// idle ones.
///
/// `last_run_ms` is `None` on a node that has genuinely never run a pass. That is honest and gets no
/// warning: there is nothing to compare against, and inventing one would be worse.
///
/// Returns how far backwards the clock has gone, when that exceeds [`SKEW_TOLERANCE_MS`].
pub fn clock_went_backwards(now_ms: u64, last_run_ms: Option<u64>) -> Option<u64> {
    let last = last_run_ms?;
    let back = last.checked_sub(now_ms)?;
    (back > SKEW_TOLERANCE_MS).then_some(back)
}

/// The operator-facing sentence for [`clock_went_backwards`].
pub fn backwards_warning(now_ms: u64, last_run_ms: Option<u64>) -> Option<String> {
    let back = clock_went_backwards(now_ms, last_run_ms)?;
    Some(format!(
        "clock went backwards: this pass's clock is {} EARLIER than this node's own last recorded \
         GC pass ({now_ms} < {}). A correct clock cannot move backwards, so this node booted with \
         the wrong time — the usual cause is no RTC battery and no NTP route, and it recurs on \
         every power cycle. Every retention horizon this pass computed is wrong. Fix the clock \
         before trusting any eviction count below.",
        humanise_ms(back),
        last_run_ms.unwrap_or(0)
    ))
}

/// The newest sample timestamp anywhere in `ws`, or `None` when the workspace holds no data.
///
/// Reads the `series_latest` POINTER table — one maintained row per series, so this is a scan of
/// "how many series exist", not of "how many samples exist". At the 1800-point sizing target that
/// is 1800 rows against millions, which is why it is affordable on every pass. The pointer is
/// forward-only and lazily backfilled ([`crate::latest`]), so a pre-pointer series contributes
/// nothing here rather than a wrong answer — the check under-reports on a legacy workspace and
/// never over-reports, which is the correct direction for something that raises an alarm.
pub async fn newest_sample_ms(store: &Store, ws: &str) -> Result<Option<u64>, StoreError> {
    let mut resp = store
        .query_ws(
            ws,
            &format!("SELECT math::max(ts) AS ts FROM {SERIES_LATEST_TABLE} GROUP ALL"),
            vec![],
        )
        .await?;
    let rows: Vec<MaxTsRow> = resp
        .take(0)
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(rows.into_iter().next().and_then(|r| r.ts))
}

/// The pointer table stores `ts` as epoch ms (the same wire form `latest` projects), so the max
/// comes back as a number. `Option` because `math::max` over an empty group is NONE.
#[derive(serde::Serialize, serde::Deserialize)]
struct MaxTsRow {
    ts: Option<u64>,
}

/// How far the DATA leads `now_ms`, when that exceeds [`SKEW_TOLERANCE_MS`].
///
/// # Why only this direction — the asymmetry is fundamental, not an omission
///
/// It is tempting to also flag a clock running AHEAD of the data, since that is the direction that
/// *over*-evicts and is therefore unrecoverable. **This signal cannot see it, and trying makes the
/// check worthless.** The two directions are not symmetric:
///
/// - **Data ahead of the clock is IMPOSSIBLE.** A committed sample's `ts` is a moment that has
///   already happened, so a newest-sample timestamp in the future is *always* a clock error. That is
///   what makes it a sound alarm.
/// - **A clock ahead of the data is ORDINARY.** It means only that nothing has been written
///   recently: a decommissioned meter, a device that is switched off, a seasonal load, a series under
///   COV that has not changed. Every one of those is a healthy node with a correct clock.
///
/// An earlier revision of this module did flag the ahead direction. The test suite caught it
/// immediately — `series_default_cap_test` seeds a series whose newest sample is ~10 days behind a
/// constant `now_ms`, which is a perfectly normal stale series, and got a spurious warning. In
/// production it would have fired on every idle series on the node, which is precisely the
/// noise-that-gets-ignored failure [`SKEW_TOLERANCE_MS`] exists to avoid.
///
/// The genuine fast-clock case is still covered, by a different and sounder signal:
/// [`clock_went_backwards`], which compares `now_ms` to this node's own last recorded pass rather
/// than to the data.
pub fn skew(now_ms: u64, newest_ms: Option<u64>) -> Option<u64> {
    let newest = newest_ms?;
    let ahead = newest.checked_sub(now_ms)?;
    (ahead > SKEW_TOLERANCE_MS).then_some(ahead)
}

/// The operator-facing sentence for a detected skew, or `None` when the clock is plausible.
///
/// Pure, so the rule is testable without a store — the I/O is [`newest_sample_ms`]'s.
pub fn skew_warning(now_ms: u64, newest_ms: Option<u64>) -> Option<String> {
    let delta = skew(now_ms, newest_ms)?;
    let newest = newest_ms?;
    Some(format!(
        "clock skew: the newest sample on disc is {} ahead of this pass's clock \
         ({newest} > {now_ms}). Retention horizons are computed from that clock, so this pass \
         evicted less than it should have — a zero eviction count here does NOT mean there was \
         nothing to evict. Check the node's clock (no RTC battery / no NTP route will do this), \
         then re-run series.retention.gc.",
        humanise_ms(delta)
    ))
}

/// A duration as the coarsest unit that keeps it readable. Only ever used inside a warning string —
/// an operator reads "46m", not "2760000".
fn humanise_ms(ms: u64) -> String {
    let secs = ms / 1_000;
    if secs < 90 {
        return format!("{secs}s");
    }
    let mins = secs / 60;
    if mins < 90 {
        return format!("{mins}m");
    }
    let hours = mins / 60;
    if hours < 48 {
        return format!("{hours}h");
    }
    format!("{}d", hours / 24)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: u64 = 60_000;

    /// The healthy shape: the clock leads the data by a little, as it must for real samples — the
    /// newest sample was written a moment ago, not in the future.
    #[test]
    fn a_clock_just_ahead_of_its_data_is_silent() {
        assert_eq!(skew_warning(1_000 * MIN, Some(999 * MIN)), None);
        assert_eq!(skew(1_000 * MIN, Some(999 * MIN)), None);
    }

    /// An empty workspace carries no lower bound on the true time, so it can say nothing. Reporting
    /// a skew here would make every fresh node cry wolf on its first pass.
    ///
    /// This is a DELIBERATE blind spot, not an oversight: a node that boots with a wrong clock and
    /// an empty series table gets no warning from here, because there is genuinely no evidence in
    /// the store to contradict it. Closing that needs a boot-time check against an independent
    /// reference, which is a different mechanism (rubix-ai#84 AC 8).
    #[test]
    fn an_empty_workspace_says_nothing() {
        assert_eq!(skew_warning(1_000 * MIN, None), None);
        assert_eq!(skew(1_000 * MIN, None), None);
    }

    /// Ordinary producer skew — a gateway stamping from its own slightly-fast clock — must not fire,
    /// or the warning becomes noise and gets filtered out, which is the failure it exists to prevent.
    #[test]
    fn producer_skew_inside_the_tolerance_is_silent() {
        assert_eq!(skew_warning(1_000 * MIN, Some(1_004 * MIN)), None);
        // Exactly at the boundary is still silent — the tolerance is inclusive.
        assert_eq!(
            skew_warning(1_000 * MIN, Some(1_000 * MIN + SKEW_TOLERANCE_MS)),
            None
        );
        // ...and symmetrically on the other side.
        assert_eq!(
            skew_warning(1_000 * MIN + SKEW_TOLERANCE_MS, Some(1_000 * MIN)),
            None
        );
    }

    /// **A stale series must NOT be mistaken for a fast clock.** The regression test for a real
    /// mistake: an earlier revision of this module flagged a clock running ahead of the data as a
    /// fault. But a clock ahead of the data is the ORDINARY state of any series that has stopped
    /// receiving — a decommissioned meter, a device switched off, a seasonal load, a COV series that
    /// has not changed. `series_default_cap_test` caught it within minutes (it seeds a series ~10
    /// days stale against a constant `now_ms`), and in production it would have fired on every idle
    /// series on the node — exactly the noise-that-gets-ignored failure the tolerance exists to
    /// prevent.
    ///
    /// Data ahead of the clock is impossible and therefore always a fault; a clock ahead of the data
    /// is normal. The asymmetry is fundamental, not an omission.
    #[test]
    fn a_long_stale_series_is_not_a_clock_fault() {
        // Ten days with no new samples: perfectly healthy, must be silent.
        assert_eq!(skew(1_000_000 * MIN, Some(985_600 * MIN)), None);
        assert_eq!(skew_warning(1_000_000 * MIN, Some(985_600 * MIN)), None);
    }

    /// The floor check is the one that CAN see a fast clock, because it compares against this node's
    /// own past rather than against data that may simply be old.
    #[test]
    fn the_floor_catches_a_clock_that_moved_backwards() {
        // A pass ran at 1000 min; the box rebooted and now claims 940 min. Impossible.
        let back = clock_went_backwards(940 * MIN, Some(1_000 * MIN)).expect("must fire");
        assert_eq!(back, 60 * MIN);

        let w = backwards_warning(940 * MIN, Some(1_000 * MIN)).expect("...and must warn");
        assert!(w.contains("clock went backwards"), "{w}");
        // "60m", not "1h" — `humanise_ms` only switches to hours past 90 minutes, so that a
        // 61-minute drift does not read as the rounder, vaguer "1h".
        assert!(w.contains("60m"), "{w}");

        // Forwards is normal — that is just time passing.
        assert_eq!(clock_went_backwards(1_060 * MIN, Some(1_000 * MIN)), None);
        // A node that never ran a pass has no floor and must not invent one.
        assert_eq!(clock_went_backwards(0, None), None);
    }

    /// The observed RC-6 failure: a 46-minute skew, which produced `evicted_raw: 0, warnings: 0`
    /// before this check existed. This test is the whole point of the module.
    #[test]
    fn the_observed_46_minute_skew_is_reported() {
        let w = skew_warning(1_000 * MIN, Some(1_046 * MIN))
            .expect("a 46m skew must produce a warning");
        assert!(w.contains("clock skew"), "{w}");
        assert!(w.contains("46m"), "the delta must be readable: {w}");
        // The message must say what a zero count means, or the operator draws the old conclusion.
        assert!(w.contains("does NOT mean"), "{w}");
    }

    #[test]
    fn durations_read_as_the_coarsest_useful_unit() {
        assert_eq!(humanise_ms(30 * 1_000), "30s");
        assert_eq!(humanise_ms(46 * MIN), "46m");
        assert_eq!(humanise_ms(5 * 60 * MIN), "5h");
        assert_eq!(humanise_ms(72 * 60 * MIN), "3d");
    }

    /// `now_ms` can legitimately be small in tests (determinism §3 stamps constants); the subtraction
    /// must never panic on the underflow that implies.
    #[test]
    fn a_zero_clock_does_not_panic() {
        assert!(skew_warning(0, Some(10 * MIN)).is_some());
        assert_eq!(skew_warning(0, Some(0)), None);
    }
}

// SurrealDB 3 reads query rows through `SurrealValue`. These delegate to serde rather than
// deriving, so `#[serde(default)]` and `deserialize_with = "de_opt_lenient_f64"` keep working
// unchanged — the derive supports neither. See `lb_store::surreal_value_via_serde!`.
lb_store::surreal_value_via_serde!(MaxTsRow);
