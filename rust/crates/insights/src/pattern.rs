//! `pattern` — the finding's SHAPE over time, derived from its own firing history
//! (`docs/scope/insights/case-plane-scope.md` §"Data model").
//!
//! `count`/`first_ts`/`last_ts` say how often and how long, but not *what kind of thing this is*.
//! A sensor that fires 400 times in a day is a different piece of work from one that fires every
//! July for three years, and a triage queue that sorts both as "47 occurrences" makes the operator
//! re-derive the distinction by eye on every row. This file derives it once, at raise, from data
//! the record already carries.
//!
//! Two pieces:
//!   - [`Insight::month_hist`](crate::Insight::month_hist) — twelve **non-evicting** counters
//!     indexed by the CALENDAR MONTH of each firing's `ts` (0 = January). Non-evicting on purpose:
//!     seasonality is only visible across years, and any ring that forgets a year cannot see it.
//!     Twelve `u32`s is ~60 bytes on the record — cheaper than any per-firing table, and it is the
//!     only history that survives the occurrence ring's eviction.
//!   - [`derive_pattern`] — a **pure** classifier over `(month_hist, count, first_ts, last_ts)`.
//!     No I/O, no clock: everything it reads is already on the record, so the same record always
//!     classifies the same way (a wall-clock read here would make the pattern drift silently
//!     between two reads of an unchanged finding).
//!
//! The calendar is **UTC**. A local calendar would need a per-workspace timezone the raise hot path
//! does not have, and the month a firing lands in only shifts at a month boundary within hours of
//! midnight — which cannot move a ≥2-year seasonality verdict. Stated because it is the kind of
//! choice that otherwise gets rediscovered as a bug.
//!
//! One responsibility: the month histogram + the pattern classifier and its thresholds.

use serde::{Deserialize, Serialize};

/// One day in the epoch-millisecond units `Insight::ts` is defined in.
pub const DAY_MS: u64 = 86_400_000;

/// Below this span a finding has not had the chance to show a shape — it is [`Pattern::New`]
/// regardless of how it fired. 30 days is one full monthly cycle of building operation: shorter
/// than that and "chronic" is indistinguishable from "the rule was deployed last week".
pub const NEW_MIN_SPAN_DAYS: u64 = 30;

/// Below this lifetime count there is no history to classify — two firings are a coincidence.
pub const NEW_MIN_COUNT: u64 = 3;

/// Firings per day, averaged over the WHOLE span, at or above which a finding is
/// [`Pattern::Flapping`]. Two a day sustained for a month is a finding nobody can act on as
/// written — it is telling you about a signal, not an event.
///
/// Averaged over the whole span deliberately, rather than over a recent window: a burst inside one
/// week of a two-year finding is an episode, not a character trait, and a window would also make
/// the verdict depend on when you asked (the drift this file's purity exists to avoid).
pub const FLAP_PER_DAY: f64 = 2.0;

/// A finding must have lived at least this long before seasonality is even a candidate — you
/// cannot see a season from inside one. Two years = two independent observations of the same
/// months.
pub const SEASONAL_MIN_SPAN_DAYS: u64 = 730;

/// "Concentrates in ≤ N calendar months": the SMALLEST set of months holding
/// [`SEASONAL_MASS`] of all firings must be no larger than this.
pub const SEASONAL_MAX_MONTHS: usize = 4;

/// The share of all firings the concentrated months must hold. 90% rather than 100% so a couple of
/// shoulder-month firings do not disqualify an obviously-summer finding, and rather than a lower
/// bar so a finding spread evenly over the year cannot squeak in on four months' worth of noise.
pub const SEASONAL_MASS: f64 = 0.90;

/// The shape of a finding's firing history. Derived on every raise by [`derive_pattern`] — never
/// caller-supplied, never stored by a producer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Pattern {
    /// Too young or too sparse to classify (see [`NEW_MIN_SPAN_DAYS`] / [`NEW_MIN_COUNT`]). The
    /// serde default, so every record written before this field landed reads as `new` rather than
    /// claiming a shape nobody derived.
    #[default]
    New,
    /// Sustained across many months — the long-running fault that never got fixed.
    Chronic,
    /// Firing far too often to be an event (see [`FLAP_PER_DAY`]).
    Flapping,
    /// Concentrated in a few calendar months, repeatedly, across years.
    Seasonal,
}

/// The empty histogram — the serde default for [`Insight::month_hist`](crate::Insight::month_hist).
pub fn empty_month_hist() -> [u32; 12] {
    [0; 12]
}

/// Is this histogram all zeroes? (The `skip_serializing_if` guard — a record that never bumped a
/// counter carries no `month_hist` key at all, so a pre-existing row round-trips byte-identically.)
pub fn is_empty_month_hist(hist: &[u32; 12]) -> bool {
    hist.iter().all(|&n| n == 0)
}

/// The calendar month (0 = January … 11 = December, UTC) of an epoch-millisecond timestamp.
///
/// Civil-from-days (Howard Hinnant's algorithm), integer-only — no `chrono`, no wall clock, and no
/// dependency on the host's timezone. `ts` is the caller's logical timestamp, so a deterministic
/// test clock classifies deterministically.
pub fn month_of_ts(ts_ms: u64) -> usize {
    // Days since 1970-01-01, shifted to the 0000-03-01 era origin the algorithm counts from.
    let z = (ts_ms / DAY_MS) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097); // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11], March-based
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12], January-based
    let _ = era; // the year is not needed — only the month
    (m - 1) as usize
}

/// Bump the calendar-month counter for a firing at `ts`. Saturating: a counter that somehow reaches
/// `u32::MAX` stops rather than wrapping to zero and inventing a brand-new finding.
pub fn bump_month(hist: &mut [u32; 12], ts_ms: u64) {
    let m = month_of_ts(ts_ms);
    hist[m] = hist[m].saturating_add(1);
}

/// Classify a finding's shape. **Pure** — every input is already on the record.
///
/// Order matters and is deliberate:
///   1. **[`Pattern::New`]** — not enough span or not enough firings. Checked first so a young
///      finding never gets a confident label off three days of data.
///   2. **[`Pattern::Flapping`]** — the rate over the whole span. Checked before seasonality
///      because a flapping finding is unusable *now*, whatever else it also is; and because the
///      whole-span average naturally excludes a genuinely seasonal finding (three months of daily
///      firings across two years averages well under [`FLAP_PER_DAY`]).
///   3. **[`Pattern::Seasonal`]** — ≥ [`SEASONAL_MIN_SPAN_DAYS`] of span AND
///      [`SEASONAL_MASS`] of all firings inside ≤ [`SEASONAL_MAX_MONTHS`] calendar months.
///      "Concentrate" is defined by construction: sort the twelve counters descending and take the
///      shortest prefix reaching the mass threshold — that prefix IS the smallest such set.
///   4. **[`Pattern::Chronic`]** — everything else: old enough, not too fast, spread across the
///      year. The long-running fault.
///
/// An empty `month_hist` (a record raised before the counters landed, until its next firing) can
/// never be seasonal — the classifier has no month data and says so by falling through to
/// `Chronic` rather than guessing.
// SCOPE: docs/scope/insights/case-plane-scope.md §"Data model" (`pattern`)
pub fn derive_pattern(month_hist: &[u32; 12], count: u64, first_ts: u64, last_ts: u64) -> Pattern {
    let span_ms = last_ts.saturating_sub(first_ts);
    let span_days = span_ms / DAY_MS;

    if span_days < NEW_MIN_SPAN_DAYS || count < NEW_MIN_COUNT {
        return Pattern::New;
    }

    // Rate over the whole span. `span_days >= NEW_MIN_SPAN_DAYS` here, so the divisor is never 0.
    let per_day = count as f64 / span_days as f64;
    if per_day >= FLAP_PER_DAY {
        return Pattern::Flapping;
    }

    if span_days >= SEASONAL_MIN_SPAN_DAYS && concentrated(month_hist) {
        return Pattern::Seasonal;
    }

    Pattern::Chronic
}

/// Does [`SEASONAL_MASS`] of the histogram sit in [`SEASONAL_MAX_MONTHS`] or fewer calendar months?
/// Sorting descending and taking the shortest prefix that reaches the mass makes that prefix the
/// smallest such set by construction — no subset search.
fn concentrated(month_hist: &[u32; 12]) -> bool {
    let total: u64 = month_hist.iter().map(|&n| n as u64).sum();
    if total == 0 {
        return false; // no month data ⇒ nothing to concentrate (never guess seasonality)
    }
    let mut counts = *month_hist;
    counts.sort_unstable_by(|a, b| b.cmp(a));
    let need = (total as f64 * SEASONAL_MASS).ceil() as u64;
    let mut acc = 0u64;
    for (i, &n) in counts.iter().enumerate() {
        acc += n as u64;
        if acc >= need {
            return i < SEASONAL_MAX_MONTHS;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Known-good calendar anchors — the algorithm is the kind of thing that is off by one month
    /// for eleven months of the year and nobody notices.
    #[test]
    fn month_of_ts_reads_the_utc_calendar() {
        assert_eq!(month_of_ts(0), 0, "1970-01-01 is January");
        // 2024-01-01T00:00:00Z = 1_704_067_200_000; 2024-07-15 = 1_721_001_600_000.
        assert_eq!(month_of_ts(1_704_067_200_000), 0);
        assert_eq!(month_of_ts(1_721_001_600_000), 6, "2024-07-15 is July");
        // 2024-12-31T23:59:59Z — the boundary the naive `ts / (30 days)` version gets wrong.
        assert_eq!(month_of_ts(1_735_689_599_000), 11);
        // 2024-02-29 — the leap day, which a 365-day-year arithmetic gets wrong.
        assert_eq!(month_of_ts(1_709_164_800_000), 1);
    }

    fn hist(months: &[(usize, u32)]) -> [u32; 12] {
        let mut h = [0u32; 12];
        for &(m, n) in months {
            h[m] = n;
        }
        h
    }

    /// Branch 1: a young or sparse finding has no shape yet.
    #[test]
    fn a_young_or_sparse_finding_is_new() {
        // Plenty of firings, but only three days of span.
        assert_eq!(
            derive_pattern(&hist(&[(0, 90)]), 90, 0, 3 * DAY_MS),
            Pattern::New,
            "span under 30 days is New however loud it is"
        );
        // Plenty of span, but two firings.
        assert_eq!(
            derive_pattern(&hist(&[(0, 1), (6, 1)]), 2, 0, 400 * DAY_MS),
            Pattern::New,
            "under 3 firings is New however old it is"
        );
    }

    /// Branch 2: rate over the whole span.
    #[test]
    fn a_high_rate_finding_is_flapping() {
        // 60 days, 300 firings = 5/day.
        assert_eq!(
            derive_pattern(&hist(&[(0, 150), (1, 150)]), 300, 0, 60 * DAY_MS),
            Pattern::Flapping
        );
        // Exactly at the threshold counts (documented as ">= FLAP_PER_DAY").
        assert_eq!(
            derive_pattern(&hist(&[(0, 120)]), 120, 0, 60 * DAY_MS),
            Pattern::Flapping
        );
    }

    /// Branch 3: concentrated in a few calendar months across years.
    #[test]
    fn a_summer_only_finding_across_two_years_is_seasonal() {
        // Three summers' worth of firings in Dec/Jan/Feb, 900 days of span, ~0.4/day.
        let h = hist(&[(11, 120), (0, 120), (1, 120)]);
        assert_eq!(derive_pattern(&h, 360, 0, 900 * DAY_MS), Pattern::Seasonal);

        // The same shape inside ONE year is not seasonal — you cannot see a season from inside one.
        assert_eq!(
            derive_pattern(&h, 360, 0, 300 * DAY_MS),
            Pattern::Chronic,
            "under two years of span, concentration is not yet seasonality"
        );

        // Spread over five months at that mass is over the concentration limit.
        let spread = hist(&[(0, 80), (1, 80), (2, 80), (3, 80), (4, 80)]);
        assert_eq!(
            derive_pattern(&spread, 400, 0, 900 * DAY_MS),
            Pattern::Chronic
        );
    }

    /// Branch 4: old, steady, spread across the year.
    #[test]
    fn a_sustained_year_round_finding_is_chronic() {
        let h = hist(&(0..12).map(|m| (m, 20)).collect::<Vec<_>>());
        assert_eq!(derive_pattern(&h, 240, 0, 400 * DAY_MS), Pattern::Chronic);
        // And with no month data at all it must NOT guess seasonality.
        assert_eq!(
            derive_pattern(&empty_month_hist(), 240, 0, 900 * DAY_MS),
            Pattern::Chronic
        );
    }

    #[test]
    fn bumping_indexes_by_calendar_month_and_never_wraps() {
        let mut h = empty_month_hist();
        bump_month(&mut h, 1_721_001_600_000); // July
        bump_month(&mut h, 1_721_001_600_000);
        assert_eq!(h[6], 2);
        assert!(is_empty_month_hist(&empty_month_hist()));
        assert!(!is_empty_month_hist(&h));

        h[6] = u32::MAX;
        bump_month(&mut h, 1_721_001_600_000);
        assert_eq!(
            h[6],
            u32::MAX,
            "saturating — never wraps to a brand-new finding"
        );
    }
}
