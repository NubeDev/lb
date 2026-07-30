//! `analysis` — the finding's own statement of the producer's REASONING
//! (`docs/scope/insights/insight-analysis-scope.md`).
//!
//! [`crate::Evidence`] says **where the data is** (datasource, plottable series, threshold,
//! window). It does not say what the producer *concluded*: why the rule fired, what metric was
//! judged, what it was compared against, how far off it was, and what it is likely to cost. Today
//! that reasoning has two wrong homes — free-form `body` (opaque, so no consumer renders it
//! consistently and no agent reads it reliably) or `title` (one line, truncated into uselessness).
//!
//! This is the second half of the evidence scope: that one made the finding state its **data**,
//! this one makes it state its **reasoning**. Beside `evidence`, never inside it — the two have
//! different lifetimes and different readers, and nesting prose in a data binding would push the
//! descriptor past its 4 KB cap for reasons that have nothing to do with SQL.
//!
//! These are the **producer's claims, stored verbatim**. The node never derives, verifies, or
//! recomputes a field; it does not check that `deviation` agrees with `evidence.threshold` and it
//! never runs a query to fill one in — the same posture `evidence` takes on SQL it never executes.
//!
//! One responsibility: the analysis shapes + their guard.

use serde::{Deserialize, Serialize};

use crate::error::InsightsError;

/// Serialized-size cap for the whole `analysis` object. Six short prose fields — a paragraph each,
/// not a report. Exceeding it rejects the WHOLE raise (never silent truncation), the contract
/// `validate_evidence_size`/`validate_occurrence_size` already hold.
pub const MAX_ANALYSIS_BYTES: usize = 4 * 1024;

/// A measured quantity: an optional number + unit **and** an optional note.
///
/// The two fields that want to be *sorted* (`deviation`, `estimated_impact`) carry this instead of
/// prose, because "rank today's findings by cost" is the first thing any report needs. Pure prose
/// (`"~$180/day"`) cannot be sorted, aggregated, or charted — and prose **cannot be backfilled into
/// numbers** later (nothing reliably parses `"3.2σ vs baseline"`), so shipping strings first would
/// leave the first year of real findings permanently unqueryable. A bare `Option<f64>` fails the
/// other way: it forces a producer to drop the honest `"N/A (data quality)"` answer that operators
/// actually read, which a plain omission loses.
///
/// So: a producer that computed nothing sets `note` only; one that computed a number sets `value` +
/// `unit` and optionally a note. A consumer sorting a roster reads `value` and skips rows without
/// one; a drawer prefers `note`, falling back to formatting `value` + `unit`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Quantity {
    /// The number, when the producer computed one. Absent = not computed / not applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    /// Unit of `value` — `"%"`, `"kL"`, `"AUD/day"`, `"sigma"`. **Required whenever `value` is
    /// set**: a bare number whose unit nobody recorded is the seed of the cross-producer
    /// unit-mismatch bug. Free text, not an enum — units are domain-open, so consistency is
    /// producer discipline and a consumer summing across producers must group by `unit` and refuse
    /// to add unlike units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// The producer's own words — the honest `"N/A (data quality)"`, or context beside a number
    /// (`"vs 1.8 kL baseline"`). Always allowed, with or without a value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl Quantity {
    /// A note-only quantity — "we considered it and it doesn't apply".
    pub fn note(note: impl Into<String>) -> Self {
        Self {
            value: None,
            unit: None,
            note: Some(note.into()),
        }
    }

    /// A measured quantity: a number in a named unit.
    pub fn measured(value: f64, unit: impl Into<String>) -> Self {
        Self {
            value: Some(value),
            unit: Some(unit.into()),
            note: None,
        }
    }

    /// Reject the two shapes that are never meaningful, so they can't reach the store: a `value`
    /// with no `unit` (an uninterpretable number), and a quantity with every field absent (an empty
    /// object that says less than omitting the field).
    fn validate(&self, field: &str) -> Result<(), InsightsError> {
        if self.value.is_some() && self.unit.is_none() {
            return Err(InsightsError::BadInput(format!(
                "analysis.{field}.value requires a unit — a bare number whose unit nobody recorded cannot be compared or summed across findings"
            )));
        }
        if self.value.is_none() && self.unit.is_none() && self.note.is_none() {
            return Err(InsightsError::BadInput(format!(
                "analysis.{field} is empty — set a `value` + `unit`, or a `note` (e.g. \"N/A (data quality)\"), or omit the field entirely"
            )));
        }
        Ok(())
    }
}

/// The producer's own explanation of the finding. Every field optional: a producer that knows only
/// its trigger logic still says something useful, and one that knows nothing omits `analysis` whole.
///
/// **CLOSED on purpose — anything outside these six belongs in `body`.** The value of these fields
/// is that *every* consumer renders the same six labels in the same order and an agent prompt can
/// name them; a free `Map<String, String>` would hand each producer a private vocabulary
/// (`root_cause` vs `rootCause` vs `cause`), which is `body` again with extra steps. The accepted
/// cost is this repo's most-repeated failure mode: **a new key is silently dropped until this Rust
/// type learns it**. That drop is deliberate, pinned by a test, and documented in the skill doc as
/// "use `body` for anything else". Escalation rule: if a *second* vertical asks for a seventh
/// field, reopen the map-vs-struct call rather than growing this struct toward ten.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Analysis {
    /// Why it fired, in the producer's words — "Zero water consumption for 24 consecutive hours".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger_logic: Option<String>,
    /// The producer's HYPOTHESIS — "Meter offline or site unoccupied (weekend)".
    ///
    /// Named `suspected_cause`, **never `root_cause`**: a rule that saw one series has not
    /// diagnosed anything, and the field name is the only thing standing between that guess and an
    /// operator who skips a site visit because "root cause" sounded settled. A UI must carry the
    /// same hedge in the *label* ("Suspected cause") — that is the one place this decision can
    /// quietly evaporate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suspected_cause: Option<String>,
    /// The metric judged, normalised — "Daily water usage (kL)".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalised_metric: Option<String>,
    /// What it was compared against — "vs expected minimum baseline".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub benchmark_context: Option<String>,
    /// How far off — `{ value: -100.0, unit: "%" }`, or note-only "N/A". Sortable by design.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deviation: Option<Quantity>,
    /// Consequence if unaddressed — `{ value: 180.0, unit: "AUD/day" }`, or note-only
    /// "N/A (data quality)". The field reports rank by.
    ///
    /// Unlike `evidence`, which points at a query you can re-run, an impact figure is
    /// unfalsifiable — and being a number it will be summed into reports and put in front of
    /// customers. Nothing on the record says how it was derived; a `body` link to the calculation
    /// is the producer's job.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_impact: Option<Quantity>,
}

/// Validate an analysis object WITHOUT writing: the per-quantity shape rules, then the whole-object
/// [`MAX_ANALYSIS_BYTES`] cap. The raise verb calls this up front (before the parent write) so an
/// oversize or malformed payload rejects the whole raise and leaves no orphan parent row — the same
/// contract `validate_evidence_size`/`validate_occurrence_size` hold.
///
/// Per-field length is bounded by the object cap only — one guard, checked once.
pub fn validate_analysis(analysis: &Analysis) -> Result<(), InsightsError> {
    if let Some(q) = &analysis.deviation {
        q.validate("deviation")?;
    }
    if let Some(q) = &analysis.estimated_impact {
        q.validate("estimated_impact")?;
    }
    let bytes = serde_json::to_vec(analysis)
        .map_err(|e| InsightsError::Store(lb_store::StoreError::Decode(e.to_string())))?;
    if bytes.len() > MAX_ANALYSIS_BYTES {
        return Err(InsightsError::BadInput(format!(
            "analysis {} bytes exceeds the {MAX_ANALYSIS_BYTES}-byte cap — analysis is six short prose fields (a paragraph each, not a report); put anything longer, and anything outside the six named fields, in `body`",
            bytes.len()
        )));
    }
    Ok(())
}
