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
#[derive(serde::Deserialize)]
struct MaxTsRow {
    ts: Option<u64>,
}

/// Compare a pass's `now_ms` against the newest data on disc and return a warning if the two
/// disagree by more than [`SKEW_TOLERANCE_MS`].
///
/// Pure, so the rule is testable without a store — the I/O is [`newest_sample_ms`]'s.
pub fn skew_warning(now_ms: u64, newest_ms: Option<u64>) -> Option<String> {
    let newest = newest_ms?;
    let ahead = newest.checked_sub(now_ms)?;
    if ahead <= SKEW_TOLERANCE_MS {
        return None;
    }
    Some(format!(
        "clock skew: the newest sample on disc is {} ahead of this pass's clock \
         ({newest} > {now_ms}). Retention horizons are computed from that clock, so this pass \
         evicted less than it should have — a zero eviction count here does NOT mean there was \
         nothing to evict. Check the node's clock (no RTC battery / no NTP route will do this), \
         then re-run series.retention.gc.",
        humanise_ms(ahead)
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

    /// The healthy shape: the clock is ahead of the data, as it must be for real samples.
    #[test]
    fn a_clock_ahead_of_the_data_is_silent() {
        assert_eq!(skew_warning(1_000 * MIN, Some(999 * MIN)), None);
    }

    /// An empty workspace carries no lower bound on the true time, so it can say nothing. Reporting
    /// a skew here would make every fresh node cry wolf on its first pass.
    #[test]
    fn an_empty_workspace_says_nothing() {
        assert_eq!(skew_warning(1_000 * MIN, None), None);
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
