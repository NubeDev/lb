//! The conversion report — the tool's **honesty contract** (grafana-conversion
//! scope, "Goals" + "Report completeness"). Every Grafana feature the audit
//! triages as mapped / degraded / dropped that appears in the input document
//! produces a line in this report. A dropped feature with no report line is a
//! test failure (see `tests/report.rs`): nothing looks silently lost.
//!
//! Grouped by the audit's three reportable buckets (the fourth — "out" — is a
//! static decision named in the scope, not a per-document line). Each line
//! carries a stable `code` (so the UI / tests can match on it) and a free-text
//! reason.

use serde::{Deserialize, Serialize};

/// One fate a mapped feature can have in the output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Fate {
    /// Cleanly mapped 1:1 onto a `Dashboard`/`Cell`/`Variable` field.
    Mapped,
    /// Preserved-but-not-rendered: carried as opaque data + an honest placeholder,
    /// flagged here so the user knows it did not round-trip cleanly.
    Degraded,
    /// Not carried at all; named so it is a decision, not a silent loss.
    Dropped,
}

/// One report line: a feature, its fate, and the reason (the audit made concrete).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportLine {
    /// Stable slug identifying the feature (e.g. `"panel.repeat"`, `"var.adhoc"`).
    pub code: String,
    pub fate: Fate,
    /// Why this fate — what the mapper did, in one sentence.
    pub reason: String,
    /// Where in the input it appeared (e.g. `"panels[2]"`, `"templating.list[0]"`),
    /// empty for dashboard-level features.
    #[serde(default)]
    pub at: String,
}

/// The per-document conversion report. Grouped by fate so the UI renders the
/// matrix the user reasons about (grafana-conversion scope, "Report surface").
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ConversionReport {
    pub mapped: Vec<ReportLine>,
    pub degraded: Vec<ReportLine>,
    pub dropped: Vec<ReportLine>,
}

impl ConversionReport {
    /// Record a `Mapped` line.
    pub fn mapped(&mut self, code: &str, at: impl Into<String>, reason: impl Into<String>) {
        self.mapped.push(ReportLine {
            code: code.to_string(),
            fate: Fate::Mapped,
            at: at.into(),
            reason: reason.into(),
        });
    }

    /// Record a `Degraded` line.
    pub fn degraded(&mut self, code: &str, at: impl Into<String>, reason: impl Into<String>) {
        self.degraded.push(ReportLine {
            code: code.to_string(),
            fate: Fate::Degraded,
            at: at.into(),
            reason: reason.into(),
        });
    }

    /// Record a `Dropped` line.
    pub fn dropped(&mut self, code: &str, at: impl Into<String>, reason: impl Into<String>) {
        self.dropped.push(ReportLine {
            code: code.to_string(),
            fate: Fate::Dropped,
            at: at.into(),
            reason: reason.into(),
        });
    }

    /// Every line, irrespective of fate (used by the completeness test).
    pub fn all(&self) -> impl Iterator<Item = &ReportLine> {
        self.mapped
            .iter()
            .chain(self.degraded.iter())
            .chain(self.dropped.iter())
    }

    /// True if any line carries `code` (used by the completeness test).
    pub fn mentions(&self, code: &str) -> bool {
        self.all().any(|l| l.code == code)
    }

    /// Total number of lines.
    pub fn len(&self) -> usize {
        self.mapped.len() + self.degraded.len() + self.dropped.len()
    }

    /// True if no lines were recorded.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_records_and_queries_lines() {
        let mut r = ConversionReport::default();
        r.mapped("panel.grid", "panels[0]", "24-col grid maps 1:1");
        r.degraded(
            "panel.repeat",
            "panels[1]",
            "preserved as raw JSON; not rendered",
        );
        r.dropped("annotations", "", "no annotation plane");
        assert_eq!(r.len(), 3);
        assert!(r.mentions("panel.repeat"));
        assert!(!r.mentions("nope"));
        assert_eq!(r.all().filter(|l| l.fate == Fate::Degraded).count(), 1);
    }
}
