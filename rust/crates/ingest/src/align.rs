//! Where a rollup bucket STARTS — the one grid a bucketed read and a GC fold both floor onto
//! (series-observability scope, Decision 21; issue #111).
//!
//! Until this slice the grid was implicit: `ts / width_ms * width_ms`, i.e. anchored to the UTC
//! epoch and nothing else. That is *right* for every width that divides a day (1 min, 5 min, 15 min,
//! 1 h all land on clean UTC wall-clock boundaries, because epoch 0 IS midnight UTC), and this
//! module keeps it EXACTLY that for a tier that declares no [`Align`] — `bucket_start(ts, w, None)`
//! is the same integer expression it always was. What it adds is the ability to say otherwise: a
//! daily tier that starts at 06:00 (a shift), or at local midnight on a site that is not on
//! Greenwich, or a 90-minute tier whose boundaries mean something.
//!
//! # The invariant this file exists to hold
//!
//! `bucket_start` is the ONLY floor in the crate. The read path ([`crate::read_buckets`], both its
//! pushdown and its fold oracle) and the write path ([`crate::run_gc`]'s rollup) call it with the
//! same `(width_ms, align)` or the tier silently mixes two griddings — a read would fold rows that
//! were stored on a different grid into buckets whose boundaries it invented, and NOTHING would
//! error. That is the failure this feature had to be designed around, not tested for afterwards; it
//! is pinned by `series_align_grid_test`, which folds through a real GC pass and reads back through
//! both paths.
//!
//! # Only the PHASE matters
//!
//! The grid is periodic with period `width_ms`, so two origins that differ by a whole number of
//! widths describe the SAME grid. Everything here therefore reduces `origin_ms` to
//! `origin_ms.rem_euclid(width_ms)` first — the phase. Three consequences worth stating, because
//! each one removes a validation rule someone would otherwise have to write:
//!   - a *negative* origin is legitimate and needs no special case (local midnight at UTC+10 is
//!     `-10 h`, which is the same grid as `+14 h`);
//!   - an origin far in the past or the future is harmless, so there is no "plausible instant"
//!     range to police — `06:00 on 1 Jan 1970` and `06:00 next Tuesday` are the same daily grid;
//!   - the arithmetic cannot overflow: the phase is bounded by the width before it is ever added to
//!     a timestamp, and the intermediate runs in `i128`.
//!
//! # DST is deliberately NOT here
//!
//! A fixed origin cannot follow a daylight-saving jump: a "daily" bucket in a DST zone drifts by an
//! hour twice a year. Carrying a real IANA zone instead would mean a variable-width grid (a DST day
//! is 23 or 25 hours long), which is not this module's `origin + k*width` model at all — it is a
//! different grid, and it would put a timezone database into a crate that is dependency-light on
//! purpose. The decision (scope Decision 21) is to ship the fixed offset, SAY so on the wire and in
//! the panel, and re-open only against a stated need. `Align` is a struct rather than a bare `i64`
//! precisely so a `tz` field can be added additively if that need arrives.

use serde::{Deserialize, Serialize};

/// Where a tier's buckets start. Absent on a [`crate::Tier`] = anchor at the UTC epoch, which is
/// this crate's behaviour since it shipped and stays byte-identical.
///
/// A bucket containing `ts` starts at `origin_ms + floor((ts - origin_ms) / width_ms) * width_ms`.
/// Only `origin_ms mod width_ms` is observable (see the module docs), so the field is best read as
/// "how far the grid is shifted from the epoch", not as a particular instant.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Align {
    /// The anchor, epoch ms. Signed: a west-of-Greenwich or before-the-epoch anchor is ordinary,
    /// and rejecting it would only force callers to add a width to it themselves.
    pub origin_ms: i64,
}

impl Align {
    /// The grid's offset from the epoch, normalized into `[0, width_ms)`.
    ///
    /// `0` for the two cases that must stay free: no alignment at all, and an origin that is already
    /// a whole number of widths from the epoch (06:00 is on the hourly grid, so an hourly tier
    /// anchored there is the epoch grid).
    pub fn phase(&self, width_ms: u64) -> u64 {
        if width_ms == 0 {
            return 0;
        }
        (self.origin_ms as i128).rem_euclid(width_ms as i128) as u64
    }
}

/// The phase of an optional alignment — `None` is the epoch grid, phase `0`.
pub fn phase_of(align: Option<Align>, width_ms: u64) -> u64 {
    align.map_or(0, |a| a.phase(width_ms))
}

/// The start of the bucket containing `ts`, on the grid `(width_ms, align)`.
///
/// With `align: None` this is `ts / width_ms * width_ms` exactly — the expression that was inline in
/// `bucket.rs` and `gc.rs` before this slice, and every existing rollup row's `t` still satisfies it.
///
/// A `width_ms` of `0` returns `ts` unchanged rather than dividing by zero. No caller can reach that
/// (`effective_width` rejects it and a tier with width `0` folds nothing), but a floor that panics is
/// a poor way to find out.
pub fn bucket_start(ts: u64, width_ms: u64, align: Option<Align>) -> u64 {
    if width_ms == 0 {
        return ts;
    }
    let phase = phase_of(align, width_ms);
    let w = width_ms as i128;
    let index = (ts as i128 - phase as i128).div_euclid(w);
    start_of_index(index, width_ms, phase)
}

/// The start of bucket `index` on the grid — the inverse of the floor, and the SAME expression the
/// pushed-down `GROUP BY` is joined back through (`bucket::raw_bucket_query` groups on the index and
/// reconstructs `t` here, so the SQL and the Rust fold cannot drift apart in the reconstruction step
/// either).
///
/// Clamped at `0`: a phase-shifted grid has a bucket that starts *before* the epoch (the one holding
/// `ts < phase`, within the first width of 1970), and bucket starts are `u64` on the wire and in the
/// rollup row id. That bucket is therefore short by `phase` ms. It is unreachable for real data —
/// the whole of it predates 2 January 1970 — and clamping keeps both paths agreeing on the one grid
/// point where the representation runs out, which is what matters.
pub fn start_of_index(index: i128, width_ms: u64, phase: u64) -> u64 {
    (index * width_ms as i128 + phase as i128).max(0) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No alignment is the pre-slice expression, exactly — not approximately.
    #[test]
    fn absent_align_is_the_epoch_floor() {
        for ts in [0u64, 1, 999, 1_000, 1_753_700_123_456] {
            for w in [1u64, 1_000, 60_000, 900_000, 86_400_000] {
                assert_eq!(bucket_start(ts, w, None), ts / w * w, "ts={ts} w={w}");
            }
        }
    }

    /// The property the whole feature rests on: a bucket start is on the grid, and it is the
    /// GREATEST such point not after `ts`.
    #[test]
    fn a_start_is_the_greatest_grid_point_at_or_before_ts() {
        let w = 900_000; // 15 min
        let align = Some(Align { origin_ms: 300_000 }); // :05 past
                                                        // From the phase upward — below it the grid runs off the front of `u64` and clamps, which
                                                        // is its own documented case (`the_pre_epoch_bucket_clamps_to_zero`).
        for ts in (300_000..7_200_000u64).step_by(37_000) {
            let start = bucket_start(ts, w, align);
            assert!(start <= ts, "start {start} > ts {ts}");
            assert!(
                ts - start < w,
                "bucket wider than {w}: ts={ts} start={start}"
            );
            assert_eq!(start % w, 300_000 % w, "start {start} off the grid");
        }
    }

    /// Only the phase is observable — the same grid can be named by infinitely many origins, and a
    /// UI that sends "06:00 today" must mean what "06:00 in 1970" means.
    #[test]
    fn origins_a_whole_width_apart_are_the_same_grid() {
        let day = 86_400_000i64;
        let six_am = 6 * 3_600_000;
        let ts = 1_753_700_123_456;
        let a = bucket_start(ts, day as u64, Some(Align { origin_ms: six_am }));
        let b = bucket_start(
            ts,
            day as u64,
            Some(Align {
                origin_ms: six_am + 900 * day,
            }),
        );
        let c = bucket_start(
            ts,
            day as u64,
            Some(Align {
                origin_ms: six_am - 900 * day,
            }),
        );
        assert_eq!(a, b);
        assert_eq!(a, c);
    }

    /// Local midnight east of Greenwich, expressed the way an operator would: a negative offset.
    /// UTC+10 → the day starts at 14:00 UTC the previous day.
    #[test]
    fn a_negative_origin_anchors_local_midnight() {
        let day = 86_400_000u64;
        let utc_plus_10 = Some(Align {
            origin_ms: -10 * 3_600_000,
        });
        // 2026-07-28T02:00:00Z == 12:00 local at UTC+10 → the local day started 2026-07-27T14:00Z.
        let ts = 1_785_204_000_000; // 2026-07-28T02:00:00Z
        let start = bucket_start(ts, day, utc_plus_10);
        assert_eq!(start, 1_785_160_800_000); // 2026-07-27T14:00:00Z
        assert_eq!((ts - start) / 3_600_000, 12); // twelve hours into the local day
                                                  // ...and the same grid named the positive way.
        let utc_plus_10_positive = Some(Align {
            origin_ms: 14 * 3_600_000,
        });
        assert_eq!(bucket_start(ts, day, utc_plus_10_positive), start);
    }

    /// A grid epoch anchoring cannot reach. NOTE which half does the work: 90 minutes *does* divide
    /// a day (24 h / 90 min = 16), so the width is not the difficulty — the PHASE is. A 06:00 anchor
    /// on a 90-minute tier is a whole number of buckets from the epoch and changes nothing; 06:30 is
    /// half a bucket off it and is reachable no other way.
    #[test]
    fn an_anchor_reaches_a_grid_the_epoch_cannot() {
        let ninety_min = 5_400_000u64;
        let day_start = 1_785_110_400_000u64; // 2026-07-27T00:00:00Z
        let ts = day_start + 7 * 3_600_000; // 07:00Z

        // 06:00 → phase 0 → the epoch grid, unchanged.
        let on_grid = Align {
            origin_ms: 6 * 3_600_000,
        };
        assert_eq!(on_grid.phase(ninety_min), 0);
        let on_grid = Some(on_grid);
        assert_eq!(
            bucket_start(ts, ninety_min, on_grid),
            bucket_start(ts, ninety_min, None)
        );

        // 06:30 → phase 30 min. Boundaries run 00:30, 02:00, … 06:30, 08:00 → 07:00 floors to 06:30.
        let shifted = Align {
            origin_ms: 6 * 3_600_000 + 30 * 60_000,
        };
        assert_eq!(shifted.phase(ninety_min), 1_800_000);
        let shifted = Some(shifted);
        assert_eq!(
            bucket_start(ts, ninety_min, shifted),
            day_start + 23_400_000
        );
        assert_ne!(
            bucket_start(ts, ninety_min, shifted),
            bucket_start(ts, ninety_min, None)
        );
    }

    /// Below the phase the grid runs off the front of `u64`; both paths must agree that it clamps.
    #[test]
    fn the_pre_epoch_bucket_clamps_to_zero() {
        let w = 60_000;
        let align = Some(Align { origin_ms: 30_000 });
        assert_eq!(bucket_start(0, w, align), 0);
        assert_eq!(bucket_start(29_999, w, align), 0);
        assert_eq!(bucket_start(30_000, w, align), 30_000);
        assert_eq!(bucket_start(89_999, w, align), 30_000);
        assert_eq!(bucket_start(90_000, w, align), 90_000);
    }

    /// A zero width is not reachable, and must not be a panic if it ever becomes so.
    #[test]
    fn a_zero_width_is_inert() {
        assert_eq!(bucket_start(1234, 0, None), 1234);
        assert_eq!(bucket_start(1234, 0, Some(Align { origin_ms: 99 })), 1234);
        assert_eq!(Align { origin_ms: 99 }.phase(0), 0);
    }
}
