//! What a case echoes from its primary insight's MONEY — the per-day rate, and the credibility
//! tier that travels with it (case-money-producer scope).
//!
//! Sibling of [`super::facets`] and the same decision restated for a number instead of a string: a
//! case's `impact_rate` is an **echo** of the primary insight's `analysis.estimated_impact`, not
//! caller input. It is split out of `facets.rs` rather than added to it because the money carries a
//! rule the facets do not — the unit has to be *checked*, and that check is the whole file.
//!
//! ## The unit is checked, never assumed
//!
//! [`lb_insights::Quantity`] carries a **free-text** `unit`, and its own doc comment says why:
//! units are domain-open, so consistency is producer discipline and *"a consumer summing across
//! producers must group by `unit` and refuse to add unlike units"*. This module is that consumer.
//! `Case.impact_rate` is a bare `f64` that the queue sums into a running total and the owner report
//! ranks by — so a `{ value: 3.2, unit: "sigma" }` deviation copied into it does not become a
//! wrong-looking number, it becomes **$3.20/day**, silently, in a customer's report. That is the
//! exact cross-producer unit-mismatch bug the `Quantity` type exists to prevent, and the only place
//! it can be prevented is here, at the one seam where a united quantity becomes a bare float.
//!
//! So: the value is copied **only** when the unit parses as a per-day currency rate
//! (`<ISO4217>/day` — see [`parse_rate_unit`]). Anything else copies **no rate at all** and logs
//! the refusal with the unit that was refused. A missing rate renders as `—` on every surface,
//! which is honest; a wrong rate does not announce itself.
//!
//! ## Rule 10
//!
//! Nothing here names a pack, a rule, a currency or an extension. `AUD/day` appears in this file
//! only as prose in a doc comment — the *code* knows the ISO-4217 SHAPE (three uppercase letters)
//! and not one member of the set. lb ships no currency table and this is not the place to grow one:
//! the workspace's currency is the workspace's business, and the case keeps whichever one its
//! producer stated.

use lb_cases::ImpactTier;
use lb_insights::Insight;

/// The suffix that makes a currency amount a **rate**. `impact_rate` is per-day by contract
/// (`case-plane-scope.md` §Money: *"cost of inaction = rate × days open"*), so a per-week or
/// per-hour figure is not a unit conversion this module may perform — it is a different number.
const PER_DAY: &str = "/day";

/// The money a case echoes from its primary insight.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct CaseImpact {
    /// The per-day rate, in `currency`. `None` whenever there is nothing honest to copy.
    pub rate: Option<f64>,
    /// How much to trust it. Present iff `rate` is.
    pub tier: Option<ImpactTier>,
}

/// Read the money echo off `insight`.
///
/// `caveated` is the primary's data-quality state, already computed by [`super::facets`]. It is
/// taken as an argument rather than recomputed so the two echoes cannot disagree about the same
/// insight: a case whose facets say caveated and whose money says otherwise is the failure this
/// avoids by construction.
///
/// **`withheld` wins over everything.** A caveated case rests on data another finding says is
/// unreliable, so it shows no figure regardless of the rate it holds — and it holds the rate all
/// the same, because the number becomes showable the moment the caveat clears and re-deriving it
/// then would mean re-reading an insight whose analysis may have moved on.
// SCOPE: docs/scope/insights/case-money-producer-scope.md §Goals 1-3
pub(super) fn impact_of(insight: &Insight, caveated: bool) -> CaseImpact {
    let Some(rate) = rate_of(insight) else {
        return CaseImpact::default();
    };
    let tier = if caveated {
        ImpactTier::Withheld
    } else {
        // `claimed`, never `modelled` — see the scope's open question 1. `modelled` rolls into the
        // owner headline and means "priced against a tariff and a baseline"; from here a rule's
        // estimate and a tariff-priced one are the same JSON, so this seam can only state the
        // weaker of the two claims honestly. A producer that earns `modelled` earns it by a later
        // pass that can tell the difference, not by this one guessing.
        ImpactTier::Claimed
    };
    CaseImpact {
        rate: Some(rate),
        tier: Some(tier),
    }
}

/// The primary's estimated impact as a per-day currency rate, or `None` with the reason logged.
fn rate_of(insight: &Insight) -> Option<f64> {
    let q = insight.analysis.as_ref()?.estimated_impact.as_ref()?;
    // A note-only quantity is the producer's honest "N/A (data quality)". Not a refusal, not a
    // warning — it is the type working as designed, so it logs nothing.
    let value = q.value?;

    let Some(unit) = q.unit.as_deref() else {
        // `Quantity::validate` already requires a unit beside a value, so this is unreachable
        // through `insight.raise`. It is handled rather than unwrapped because a record written
        // before that validation landed is still readable, and a bare number whose unit nobody
        // recorded is precisely the thing this module must not copy.
        tracing::warn!(
            insight = %insight.id,
            "estimated_impact has a value and no unit; no rate copied to the case"
        );
        return None;
    };

    let Some(currency) = parse_rate_unit(unit) else {
        tracing::info!(
            insight = %insight.id, %unit,
            "estimated_impact is not a per-day currency rate; no rate copied to the case \
             (a case's impact_rate is money per day — see case-money-producer-scope.md)"
        );
        return None;
    };

    if !value.is_finite() {
        tracing::warn!(
            insight = %insight.id, %currency,
            "estimated_impact value is not finite; no rate copied to the case"
        );
        return None;
    }

    tracing::debug!(insight = %insight.id, %currency, value, "case impact rate echoed");
    Some(value)
}

/// The currency in `unit`, when `unit` is a per-day currency rate.
///
/// The grammar is `<ISO4217>/day`: exactly three ASCII letters, uppercase, then the literal
/// `/day`. A **pattern, not an allowlist** — an allowlist is a currency table with 180 rows that
/// goes stale, that lb has no business owning (`case-money-producer-scope.md` non-goal: *"not a
/// currency model"*), and that would refuse a perfectly good workspace for being somewhere lb had
/// not heard of. The shape is enough to separate money from `sigma`, `kL`, `%` and `kWh`, which is
/// the whole job.
///
/// **Case-sensitive.** ISO 4217 codes are uppercase, and accepting `aud/day` would mean accepting
/// `abc/day` with the same confidence — at which point the check is "three letters", which `kWh`
/// very nearly passes. A producer that means money can write it the way the standard writes it.
fn parse_rate_unit(unit: &str) -> Option<&str> {
    let currency = unit.strip_suffix(PER_DAY)?;
    let ok = currency.len() == 3 && currency.bytes().all(|b| b.is_ascii_uppercase());
    ok.then_some(currency)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lb_insights::{Analysis, Quantity};

    /// An insight carrying `analysis` — the only field these tests vary.
    ///
    /// Built by DESERIALIZING the wire form rather than by a struct literal, for two reasons: it is
    /// the path a stored record actually takes (`lb_insights::get` decodes exactly this), and it
    /// means a new required field on `Insight` does not edit this file.
    fn insight_with(analysis: Option<Analysis>) -> Insight {
        let mut value = serde_json::json!({
            "id": "insight-1",
            "dedup_key": "test:1",
            "severity": "warning",
            "title": "a finding",
            "origin": { "kind": "rule", "ref": "test-rule" },
            "status": "open",
            "count": 1,
            "first_ts": 0,
            "last_ts": 0,
            "producer": "test",
        });
        if let Some(a) = analysis {
            value["analysis"] = serde_json::to_value(a).expect("analysis serializes");
        }
        serde_json::from_value(value).expect("the fixture decodes as an Insight")
    }

    /// An insight whose `analysis.estimated_impact` is `q`.
    fn with_impact(q: Option<Quantity>) -> Insight {
        insight_with(Some(Analysis {
            estimated_impact: q,
            ..Default::default()
        }))
    }

    #[test]
    fn a_per_day_currency_rate_is_copied_and_claimed() {
        let got = impact_of(
            &with_impact(Some(Quantity::measured(180.0, "AUD/day"))),
            false,
        );
        assert_eq!(
            got,
            CaseImpact {
                rate: Some(180.0),
                tier: Some(ImpactTier::Claimed),
            }
        );
    }

    #[test]
    fn a_note_only_quantity_yields_no_rate_and_no_tier() {
        let got = impact_of(
            &with_impact(Some(Quantity::note("N/A (data quality)"))),
            false,
        );
        assert_eq!(got, CaseImpact::default());
    }

    /// The bug the unit check exists for: a deviation in sigma must not become $3.20/day.
    #[test]
    fn an_unlike_unit_is_refused() {
        for unit in [
            "sigma", "kL", "%", "kWh", "AUD", "AUD/week", "AUD/hour", "aud/day", "AUDX/day",
            "AU/day",
        ] {
            let got = impact_of(&with_impact(Some(Quantity::measured(3.2, unit))), false);
            assert_eq!(
                got,
                CaseImpact::default(),
                "unit {unit:?} should be refused"
            );
        }
    }

    #[test]
    fn any_iso_shaped_currency_is_accepted_lb_ships_no_currency_table() {
        for unit in ["AUD/day", "USD/day", "GBP/day", "JPY/day", "ZWL/day"] {
            let got = impact_of(&with_impact(Some(Quantity::measured(1.0, unit))), false);
            assert_eq!(got.rate, Some(1.0), "unit {unit:?} should be accepted");
        }
    }

    #[test]
    fn no_analysis_at_all_yields_no_rate() {
        assert_eq!(impact_of(&insight_with(None), false), CaseImpact::default());
        assert_eq!(impact_of(&with_impact(None), false), CaseImpact::default());
    }

    /// Goal 3. The rate is KEPT — it becomes showable when the caveat clears — but the tier says
    /// withheld, and every consumer's `withheld` rule turns that into no figure.
    #[test]
    fn a_caveated_primary_withholds_a_real_rate() {
        let got = impact_of(
            &with_impact(Some(Quantity::measured(420.0, "AUD/day"))),
            true,
        );
        assert_eq!(
            got,
            CaseImpact {
                rate: Some(420.0),
                tier: Some(ImpactTier::Withheld),
            }
        );
    }

    /// A caveated primary with NO priceable impact stays entirely untiered — `withheld` describes a
    /// number we are holding back, and there is no number here to hold back.
    #[test]
    fn a_caveated_primary_with_no_rate_is_untiered_not_withheld() {
        let got = impact_of(&with_impact(Some(Quantity::note("N/A"))), true);
        assert_eq!(got, CaseImpact::default());
    }

    #[test]
    fn a_non_finite_value_is_refused() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let got = impact_of(
                &with_impact(Some(Quantity::measured(value, "AUD/day"))),
                false,
            );
            assert_eq!(got, CaseImpact::default());
        }
    }
}
