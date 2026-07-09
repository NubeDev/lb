//! grafana-conv-mapper — the pure `grafana_json → (Dashboard, ConversionReport)`
//! converter. The headline deliverable of the grafana-conv umbrella (Stage 0).
//!
//! # Example
//!
//! ```no_run
//! use grafana_conv_mapper::convert;
//! let json = std::fs::read_to_string("dashboard.json").unwrap();
//! let (dash, report) = convert(&json).expect("parse");
//! println!("{}", serde_json::to_string_pretty(&dash).unwrap());
//! println!("-- report ({} lines) --", report.len());
//! ```
//!
//! See the scope: `docs/scope/frontend/dashboard/grafana-conversion-scope.md`.

mod input;
#[rustfmt::skip]
#[allow(dead_code, clippy::derivable_impls)]
// the mirror deliberately mirrors the host's full surface, not just the bits we emit
mod model;
mod panels;
mod report;
mod settings;
mod variables;

pub use input::{GrafanaDashboard, GridPos, Panel, Templating, Variable as GrafanaVariable};
pub use model::{Cell, Dashboard, Target, Variable};
pub use report::{ConversionReport, Fate, ReportLine};

use thiserror::Error;

/// A conversion failure. Parse errors carry the serde message; structural errors
/// (v2 input rejected) are a distinct variant so the UI can show "unsupported".
#[derive(Debug, Error)]
pub enum ConvertError {
    #[error("invalid Grafana JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("unsupported input: {0}")]
    Unsupported(String),
}

/// The conversion result.
pub struct Converted {
    pub dashboard: Dashboard,
    pub report: ConversionReport,
}

/// Convert a Grafana dashboard JSON document to our `Dashboard` + a per-document
/// report of every mapped / degraded / dropped feature. Pure — no I/O.
pub fn convert(input: &str) -> Result<(Dashboard, ConversionReport), ConvertError> {
    let raw: serde_json::Value = serde_json::from_str(input)?;

    // Strip the `/api/dashboards/uid` envelope (`{"dashboard": {...}, meta}`) if present.
    let dash_value = match &raw {
        serde_json::Value::Object(map) if map.contains_key("dashboard") => {
            raw.get("dashboard").cloned().unwrap_or(raw)
        }
        _ => raw,
    };

    let g: GrafanaDashboard = serde_json::from_value(dash_value)?;
    if g.looks_like_v2() {
        return Err(ConvertError::Unsupported(
            "Grafana v2 (`dashboard.grafana.app/v2beta1`) kind-based layout is not accepted \
             (classic `panels[]` schemaVersion 42 only)"
                .into(),
        ));
    }

    let mut report = ConversionReport::default();
    let settings::DashSettings {
        id,
        title,
        description,
    } = settings::map_settings(&g, &mut report);
    let cells = panels::map_cells(&g, &mut report);
    let variables = variables::map_variables(&g, &mut report);

    let dash = Dashboard {
        id,
        title,
        description,
        icon: String::new(),
        color: String::new(),
        owner: String::new(), // standalone tool: no principal; set on fold-in.
        visibility: Default::default(),
        cells,
        variables,
        schema_version: model::SCHEMA_VERSION,
        updated_ts: 0, // standalone tool: no clock; set on fold-in.
        deleted: false,
    };

    Ok((dash, report))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_round_trips() {
        let (dash, report) = convert("{}").expect("empty");
        assert_eq!(dash.id, "imported");
        assert!(dash.cells.is_empty());
        assert!(dash.variables.is_empty());
        // title is always reported mapped (it maps even when empty).
        assert!(report.mentions("dashboard.title"));
        assert!(!report.mentions("panel.grid"));
    }

    #[test]
    fn envelope_is_stripped() {
        let wrapped = r#"{"dashboard": {"title": "X"}, "meta": {}}"#;
        let (dash, _) = convert(wrapped).expect("envelope");
        assert_eq!(dash.title, "X");
    }

    #[test]
    fn bare_title_maps() {
        let (dash, report) = convert(r#"{"title": "My Dash"}"#).expect("title");
        assert_eq!(dash.title, "My Dash");
        assert_eq!(dash.id, "my-dash");
        assert!(report.mentions("dashboard.title"));
    }
}
