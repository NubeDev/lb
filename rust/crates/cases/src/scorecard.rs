//! `scorecard` — the **pure** arithmetic behind "how good is this detector?"
//! (case-plane scope §7, "Feedback from resolution to detection").
//!
//! `origin.ref` names the producer; `case.resolution` names what happened when a human finished the
//! work. Joining the two per `(rule_ref, site)` is the whole feedback loop: a detector that keeps
//! producing false positives at one site is visible as a NUMBER instead of as a mood.
//!
//! **Pure on purpose.** Everything here is arithmetic over already-loaded rows — no store, no clock,
//! no I/O — so the formula that will be argued about in a review is unit-testable in microseconds
//! and the host layer above owns every read. Rule 10 holds trivially: `rule_ref` and `site` are
//! opaque strings this file GROUPS BY and never parses, matches or prefixes.
//!
//! **What v1 deliberately does NOT do.** The scope's `rule_policy` demotion — "under
//! `precision_floor` over `min_outcomes`, raise at Info" — is an explicit fast-follow and is not
//! computed, suggested or applied anywhere here. This file reports; nothing acts on the report.
//! A number that silently changes how a detector fires is exactly the "silent code" §7 rules out.

use serde::{Deserialize, Serialize};

use crate::case::Resolution;

/// One resolved case, flattened to the five fields the arithmetic needs.
///
/// The host builds these by scanning resolved cases and resolving each case's `primary_insight` to
/// its `origin.ref`; this crate never learns how that join is done.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedCase {
    /// The opaque producer id (`origin.ref`) the case's primary insight came from.
    pub rule_ref: String,
    /// The site facet, or `None` for a case with no site. A `None` site is a real group, not a
    /// missing one — see [`scorecard`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site: Option<String>,
    /// How the case closed.
    pub resolution: Resolution,
    /// When the case was opened (epoch ms).
    pub opened_ts: u64,
    /// When it was resolved (epoch ms).
    pub resolved_ts: u64,
}

/// The scorecard for one `(rule_ref, site)` pair.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScorecardRow {
    /// The opaque producer id these outcomes belong to.
    pub rule_ref: String,
    /// The site, or `None` for the no-site group.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site: Option<String>,
    /// How many resolved cases this row covers — the sum of the five outcome counts.
    pub raised: u64,
    /// Resolved [`Resolution::Fixed`].
    pub fixed: u64,
    /// Resolved [`Resolution::FalsePositive`].
    pub false_positive: u64,
    /// Resolved [`Resolution::SelfCleared`].
    pub self_cleared: u64,
    /// Resolved [`Resolution::AcceptedRisk`].
    pub accepted_risk: u64,
    /// Resolved [`Resolution::Duplicate`].
    pub duplicate: u64,
    /// `fixed / (fixed + false_positive + self_cleared)`, or **`None` when that denominator is 0**.
    ///
    /// `None` means "no opinion", and it is NOT the same value as `0.0`. Rendering `0.0` for a
    /// detector nobody has finished a case for yet would defame it with a number it never earned —
    /// so the absence is carried in the type and a UI must render it as a dash, never as zero.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub precision: Option<f64>,
    /// Median `resolved_ts - opened_ts` over the **`fixed`** cases only, in milliseconds. `None`
    /// when the row has no `fixed` case.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub median_resolve_ms: Option<u64>,
}

/// Fold resolved cases into one row per `(rule_ref, site)`.
///
/// **The precision formula is `fixed / (fixed + false_positive + self_cleared)`.** The three
/// outcomes in the denominator are the ones that say something about the DETECTION:
///   - `fixed` — the detector was right and the work was real;
///   - `false_positive` — the detector was wrong;
///   - `self_cleared` — nothing was done and it went away, which is the honest form of "there was
///     no work here", i.e. the detector cried before there was anything to do.
///
/// **`accepted_risk` and `duplicate` are counted and reported but are NOT in the denominator**, and
/// this is the line a reader will want to argue with, so: accepting a risk is a BUDGET decision
/// about a fault everyone agrees is real — the detector was right and got marked down for it if it
/// counted against precision. And `duplicate` is a statement about the GROUPING (two cases were one
/// piece of work), not about the detection; a storm that folds twenty cases into one would tank a
/// perfectly good detector's score. Both are still reported, because "this rule's findings are
/// always accepted-risk" is a real signal — it just is not a precision signal.
///
/// **Median time-to-resolve is over the `fixed` cases only.** A false positive's "resolve time"
/// measures how long it took somebody to NOTICE it was noise, and a self-cleared case's measures
/// how long the fault happened to last — different quantities in the same unit, and mixing them in
/// drags the median into meaninglessness. The number is meant to answer "how long does fixing one
/// of these take?", so only the cases where something was fixed may vote.
///
/// **Even counts take the MEAN of the two middle values**, not the lower. With two samples the
/// midpoint is what a reader means by "the middle"; always taking the lower would bias every
/// even-count row downwards, and under-reporting how long work takes is the direction that gets
/// people committed to deadlines they cannot meet.
///
/// A case with no site groups under `site: None` and is **never dropped** — a silently discarded
/// row is how a precision number becomes a lie.
///
/// Rows come back sorted by `(rule_ref, site)` with the `None` site first, so a caller gets a
/// stable order without re-sorting and two runs over the same data are byte-identical.
// SCOPE: docs/scope/insights/case-plane-scope.md §"Verbs" (rule.scorecard)
pub fn scorecard(rows: &[ResolvedCase]) -> Vec<ScorecardRow> {
    // A BTreeMap, so the output order is the key order and needs no separate sort pass. `Option`
    // orders `None` before `Some`, which puts the no-site group first — a deliberate, documented
    // position rather than an accident of hashing.
    let mut groups: std::collections::BTreeMap<(String, Option<String>), Group> =
        Default::default();

    for row in rows {
        let group = groups
            .entry((row.rule_ref.clone(), row.site.clone()))
            .or_default();
        group.raised += 1;
        match row.resolution {
            Resolution::Fixed => {
                group.fixed += 1;
                // Saturating, because a record whose `resolved_ts` predates its `opened_ts` (a
                // clock that moved, a hand-edited row) must contribute 0 rather than wrap to
                // ~584 million years and swallow the median whole.
                group
                    .fixed_durations
                    .push(row.resolved_ts.saturating_sub(row.opened_ts));
            }
            Resolution::FalsePositive => group.false_positive += 1,
            Resolution::SelfCleared => group.self_cleared += 1,
            Resolution::AcceptedRisk => group.accepted_risk += 1,
            Resolution::Duplicate => group.duplicate += 1,
        }
    }

    groups
        .into_iter()
        .map(|((rule_ref, site), mut g)| {
            let denominator = g.fixed + g.false_positive + g.self_cleared;
            ScorecardRow {
                rule_ref,
                site,
                raised: g.raised,
                fixed: g.fixed,
                false_positive: g.false_positive,
                self_cleared: g.self_cleared,
                accepted_risk: g.accepted_risk,
                duplicate: g.duplicate,
                precision: (denominator > 0).then(|| g.fixed as f64 / denominator as f64),
                median_resolve_ms: median(&mut g.fixed_durations),
            }
        })
        .collect()
}

/// The per-group accumulator. Private: the shape callers see is [`ScorecardRow`].
#[derive(Default)]
struct Group {
    raised: u64,
    fixed: u64,
    false_positive: u64,
    self_cleared: u64,
    accepted_risk: u64,
    duplicate: u64,
    fixed_durations: Vec<u64>,
}

/// The median of `values`, or `None` when empty. Even counts take the mean of the two middle
/// values (see [`scorecard`]); the mean is computed in `u128` so two large millisecond durations
/// cannot overflow on the way to their midpoint.
fn median(values: &mut [u64]) -> Option<u64> {
    if values.is_empty() {
        return None;
    }
    values.sort_unstable();
    let mid = values.len() / 2;
    if values.len() % 2 == 1 {
        Some(values[mid])
    } else {
        Some(((values[mid - 1] as u128 + values[mid] as u128) / 2) as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(
        rule: &str,
        site: Option<&str>,
        resolution: Resolution,
        opened: u64,
        resolved: u64,
    ) -> ResolvedCase {
        ResolvedCase {
            rule_ref: rule.into(),
            site: site.map(str::to_string),
            resolution,
            opened_ts: opened,
            resolved_ts: resolved,
        }
    }

    /// The formula, exactly: 2 fixed against 1 false positive and 1 self-cleared ⇒ 0.5.
    #[test]
    fn precision_is_fixed_over_fixed_plus_false_positive_plus_self_cleared() {
        let rows = vec![
            row("r", Some("s"), Resolution::Fixed, 0, 10),
            row("r", Some("s"), Resolution::Fixed, 0, 20),
            row("r", Some("s"), Resolution::FalsePositive, 0, 5),
            row("r", Some("s"), Resolution::SelfCleared, 0, 5),
        ];
        let out = scorecard(&rows);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].precision, Some(0.5));
        assert_eq!(out[0].raised, 4);
    }

    /// `accepted_risk` and `duplicate` are counted but stay OUT of the denominator — the detector
    /// was right in both cases and must not be marked down for a budget call or a merge.
    #[test]
    fn accepted_risk_and_duplicate_are_counted_but_never_in_the_denominator() {
        let rows = vec![
            row("r", None, Resolution::Fixed, 0, 10),
            row("r", None, Resolution::AcceptedRisk, 0, 10),
            row("r", None, Resolution::Duplicate, 0, 10),
        ];
        let out = scorecard(&rows);
        assert_eq!(out[0].accepted_risk, 1);
        assert_eq!(out[0].duplicate, 1);
        assert_eq!(out[0].raised, 3);
        // 1 / (1 + 0 + 0) — the two extra outcomes moved nothing.
        assert_eq!(out[0].precision, Some(1.0));
    }

    /// A row with only excluded outcomes has a ZERO denominator, and the answer is "no opinion" —
    /// never `0.0`, which would read as "this detector is always wrong".
    #[test]
    fn a_zero_denominator_is_none_not_zero() {
        let rows = vec![
            row("r", None, Resolution::AcceptedRisk, 0, 10),
            row("r", None, Resolution::Duplicate, 0, 10),
        ];
        let out = scorecard(&rows);
        assert_eq!(out[0].precision, None);
        assert_ne!(out[0].precision, Some(0.0));
        // And it is not a NaN wearing a `Some`.
        assert!(!out[0].precision.is_some_and(f64::is_nan));
    }

    /// A detector that is genuinely always wrong DOES score 0.0 — the `None` above is about an
    /// empty denominator, not about a bad score.
    #[test]
    fn an_always_wrong_detector_scores_zero_not_none() {
        let rows = vec![row("r", None, Resolution::FalsePositive, 0, 10)];
        assert_eq!(scorecard(&rows)[0].precision, Some(0.0));
    }

    #[test]
    fn median_of_an_odd_count_is_the_middle_value() {
        let rows = vec![
            row("r", None, Resolution::Fixed, 0, 100),
            row("r", None, Resolution::Fixed, 0, 300),
            row("r", None, Resolution::Fixed, 0, 200),
        ];
        assert_eq!(scorecard(&rows)[0].median_resolve_ms, Some(200));
    }

    #[test]
    fn median_of_an_even_count_is_the_mean_of_the_two_middle_values() {
        let rows = vec![
            row("r", None, Resolution::Fixed, 0, 100),
            row("r", None, Resolution::Fixed, 0, 200),
            row("r", None, Resolution::Fixed, 0, 300),
            row("r", None, Resolution::Fixed, 0, 500),
        ];
        // (200 + 300) / 2, not 200.
        assert_eq!(scorecard(&rows)[0].median_resolve_ms, Some(250));
    }

    /// Only `fixed` cases vote on the median: a 10-second false positive beside a 100-second fix
    /// must not pull the "how long does a fix take" number down to 55.
    #[test]
    fn only_fixed_cases_vote_on_the_median() {
        let rows = vec![
            row("r", None, Resolution::Fixed, 0, 100),
            row("r", None, Resolution::FalsePositive, 0, 10),
            row("r", None, Resolution::SelfCleared, 0, 10),
        ];
        assert_eq!(scorecard(&rows)[0].median_resolve_ms, Some(100));
    }

    /// No `fixed` case ⇒ no median. Same discipline as `precision`: absence, not zero.
    #[test]
    fn no_fixed_case_means_no_median() {
        let rows = vec![row("r", None, Resolution::FalsePositive, 0, 10)];
        assert_eq!(scorecard(&rows)[0].median_resolve_ms, None);
    }

    /// A `resolved_ts` before the `opened_ts` contributes 0, not a wrapped near-eternity.
    #[test]
    fn a_backwards_duration_saturates_to_zero() {
        let rows = vec![row("r", None, Resolution::Fixed, 500, 100)];
        assert_eq!(scorecard(&rows)[0].median_resolve_ms, Some(0));
    }

    /// The group key is the PAIR: one rule at two sites is two rows, and two rules at one site is
    /// two rows.
    #[test]
    fn grouping_is_by_rule_ref_and_site_together() {
        let rows = vec![
            row("a", Some("north"), Resolution::Fixed, 0, 10),
            row("a", Some("south"), Resolution::FalsePositive, 0, 10),
            row("b", Some("north"), Resolution::Fixed, 0, 10),
        ];
        let out = scorecard(&rows);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].rule_ref, "a");
        assert_eq!(out[0].site.as_deref(), Some("north"));
        assert_eq!(out[0].precision, Some(1.0));
        assert_eq!(out[1].site.as_deref(), Some("south"));
        assert_eq!(out[1].precision, Some(0.0));
        assert_eq!(out[2].rule_ref, "b");
    }

    /// A case with no site is its OWN group and survives — it is never folded into a sited row and
    /// never dropped.
    #[test]
    fn the_no_site_group_survives_and_sorts_first() {
        let rows = vec![
            row("a", Some("north"), Resolution::Fixed, 0, 10),
            row("a", None, Resolution::FalsePositive, 0, 10),
        ];
        let out = scorecard(&rows);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].site, None, "the None group sorts first");
        assert_eq!(out[0].false_positive, 1);
        assert_eq!(out[1].site.as_deref(), Some("north"));
        assert_eq!(out[1].fixed, 1);
    }

    /// No rows in, no rows out — an empty scorecard, not a panic and not a zero row.
    #[test]
    fn no_rows_is_an_empty_scorecard() {
        assert!(scorecard(&[]).is_empty());
    }
}
