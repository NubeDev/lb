//! `evidence` — the finding's own statement of the data that proves it
//! (`docs/scope/insights/insight-evidence-scope.md`).
//!
//! A rule runs a query, judges the result, and raises. Without this field the query dies at the
//! raise boundary, and every consumer that wants to draw "the finding on its trend" is forced to
//! guess a series out of the free-form `body` — a guess that is wrong for whole classes of rule.
//! `evidence` is the narrow, machine-readable binding beside `body`: the datasource, the plottable
//! series, and the threshold/window judged.
//!
//! **`series` is not the judgment query.** The rule's own SQL is frequently an aggregate — a
//! `GROUP BY` yielding one scalar per entity, with no time axis — which plots as nothing. The
//! trend an operator wants is the underlying per-entity series, a *different* query the producer
//! can write but the engine cannot derive. Hence the split: [`Evidence::series`] is what you draw,
//! [`Evidence::query`] is provenance for what was judged, and only `series` is needed to render.
//!
//! This is a DESCRIPTOR, not data and not an executor: the node never runs, plans, or validates
//! these queries. Executing one is the reader's business, through the data plane's own verb and
//! that verb's own caps and workspace wall. A malformed series is a broken panel, not a failed
//! raise. Per-firing sample data has a different home — the occurrence ring.
//!
//! One responsibility: the evidence descriptor shapes + their size guard.

use serde::{Deserialize, Serialize};

use crate::error::InsightsError;

/// Serialized-size cap for the whole `evidence` object. Sized for a few joins' worth of SQL, by
/// analogy with the occurrence ring's 2 KB `data` cap. Exceeding it rejects the WHOLE raise (never
/// silent truncation) — evidence is a descriptor, and a payload this large means sample data is
/// being smuggled into it.
pub const MAX_EVIDENCE_BYTES: usize = 4 * 1024;

/// The default data-plane verb the series dispatch through when a producer names none.
pub const DEFAULT_TOOL: &str = "federation.query";

/// One plottable series the finding sits on — a query yielding `(time, value)` rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSeries {
    /// The query yielding `(time, value)` rows. Dialect is the datasource's business, not ours.
    pub sql: String,
    /// Display label for the series (legend/tooltip). Absent ⇒ the reader names it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Unit of `value` (`"kWh"`, `"degC"`) — the reader's axis/threshold formatting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
}

impl EvidenceSeries {
    /// A bare series with no label or unit — the shape the string sugar decodes to.
    pub fn new(sql: impl Into<String>) -> Self {
        Self {
            sql: sql.into(),
            label: None,
            unit: None,
        }
    }
}

/// Accepts either a bare SQL string or a full [`EvidenceSeries`] object, so a rhai rule with one
/// unlabelled series can write `series: ["SELECT …"]` instead of `series: [#{ sql: "SELECT …" }]`.
/// Serializes back as the full object — one stored shape, two authoring shapes.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
enum SeriesInput {
    Sql(String),
    Full(EvidenceSeries),
}

impl From<SeriesInput> for EvidenceSeries {
    fn from(v: SeriesInput) -> Self {
        match v {
            SeriesInput::Sql(sql) => EvidenceSeries::new(sql),
            SeriesInput::Full(s) => s,
        }
    }
}

fn de_series<'de, D>(d: D) -> Result<Vec<EvidenceSeries>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = <Vec<SeriesInput>>::deserialize(d)?;
    Ok(raw.into_iter().map(EvidenceSeries::from).collect())
}

/// The window the producer judged, as epoch-ms — lets a reader open the trend pre-ranged to the
/// span the finding is actually about, rather than a default range that may not contain it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceWindow {
    pub from: u64,
    pub to: u64,
}

/// The finding's data binding. Every field but `source` is optional: a producer that knows only
/// its datasource still says something useful, and one that knows nothing omits `evidence` whole.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    /// Datasource id the series resolve against (e.g. a federation source name). Resolved by the
    /// reader at read time, per-workspace — never resolved or validated here.
    pub source: String,
    /// The plottable series the finding sits on. A LIST because findings are routinely
    /// multi-series (a cooling-failure judges supply *and* return temperature, and the trend that
    /// proves it must draw both), and because a list maps 1:1 onto a panel's `sources[]`.
    #[serde(
        default,
        deserialize_with = "de_series",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub series: Vec<EvidenceSeries>,
    /// The judgment query — what the producer actually ran to decide. Provenance / "open evidence"
    /// only; frequently NOT plottable (see the module doc). Never required to render.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    /// The window judged (epoch-ms).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<EvidenceWindow>,
    /// The threshold crossed, in the series' own units — drawn as a threshold line so the eye
    /// reads *line crosses threshold → the finding fired*.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f64>,
    /// Data-plane verb the series dispatch through. Present so a producer on another data plane is
    /// not locked out; absent ⇒ [`DEFAULT_TOOL`]. Never a consumer id (rule 10).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    /// The **point references** this finding was derived from — opaque strings to lb, minted and
    /// interpreted by the producer (the case plane's caveat stamp reads them).
    ///
    /// lb never resolves, validates, or dereferences one. What it DOES with them is set-intersect:
    /// two findings that name a subject in common are findings about the same physical thing, which
    /// is what lets an open data-quality finding caveat everything derived from the same points
    /// (`caveat.rs`), and what lets a later grouping pass fold by topology. That is the whole
    /// contract — a subject is an equality token, so a producer must only be *consistent*, never
    /// canonical, and lb learns nothing about what a point is (rule 10).
    ///
    /// A LIST because a finding routinely rests on several points (a cooling failure judges supply
    /// AND return), and because intersection is the operation.
    ///
    /// Additive: absent on every record written before the field landed and on every producer that
    /// states none. Counted by [`validate_evidence_size`] like every other field on the object —
    /// the cap is on the whole descriptor, so a producer cannot smuggle a point inventory in here.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subjects: Vec<String>,
}

impl Evidence {
    /// The verb these series dispatch through — the producer's choice, else [`DEFAULT_TOOL`].
    pub fn tool(&self) -> &str {
        self.tool.as_deref().unwrap_or(DEFAULT_TOOL)
    }
}

/// Validate an evidence descriptor against [`MAX_EVIDENCE_BYTES`] WITHOUT writing. The raise verb
/// calls this up front (before the parent write) so an oversize payload rejects the whole raise and
/// leaves no orphan parent row — the same contract `validate_occurrence_size` holds for the ring.
pub fn validate_evidence_size(evidence: &Evidence) -> Result<(), InsightsError> {
    let bytes = serde_json::to_vec(evidence)
        .map_err(|e| InsightsError::Store(lb_store::StoreError::Decode(e.to_string())))?;
    if bytes.len() > MAX_EVIDENCE_BYTES {
        return Err(InsightsError::BadInput(format!(
            "evidence {} bytes exceeds the {MAX_EVIDENCE_BYTES}-byte cap — evidence is a descriptor (source + query), not sample data; per-firing rows belong in the occurrence ring",
            bytes.len()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Evidence {
        Evidence {
            source: "demo".into(),
            series: vec![EvidenceSeries::new("SELECT t, v FROM s")],
            query: None,
            window: None,
            threshold: None,
            tool: None,
            subjects: Vec::new(),
        }
    }

    /// **The guard that would go red without the fix**: `subjects` is serialized as part of the
    /// object, so the 4 KB cap must count it. Written as "a descriptor with a plausible handful of
    /// subjects passes; one carrying a point INVENTORY is rejected whole" — the abuse the cap
    /// exists for, now reachable through a new field.
    #[test]
    fn the_size_guard_counts_subjects() {
        let ok = Evidence {
            subjects: (0..8).map(|i| format!("point:ahu-2/sat-{i}")).collect(),
            ..base()
        };
        assert!(validate_evidence_size(&ok).is_ok());

        let inventory = Evidence {
            subjects: (0..400)
                .map(|i| format!("point:site-a/meter-{i:04}/energy"))
                .collect(),
            ..base()
        };
        let err = validate_evidence_size(&inventory).expect_err("over the cap");
        assert!(
            err.to_string().contains("evidence"),
            "the error names the field: {err}"
        );
    }

    /// Additive decode: a stored descriptor written BEFORE `subjects` existed still decodes, and
    /// an absent list serializes away entirely (no new key on any existing record).
    #[test]
    fn a_pre_existing_descriptor_decodes_and_round_trips_without_the_key() {
        let old = serde_json::json!({ "source": "demo", "series": ["SELECT t, v FROM s"] });
        let ev: Evidence = serde_json::from_value(old).expect("pre-existing shape decodes");
        assert!(ev.subjects.is_empty());
        let back = serde_json::to_value(&ev).unwrap();
        assert!(back.get("subjects").is_none(), "empty ⇒ no key: {back}");
    }
}
