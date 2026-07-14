//! The ported v33 migration subset, ordered — the pin's floor, not the full `DashboardMigrator` chain.
//!
//! Grafana's migrator walks every schema version from the export's `schemaVersion` up to current,
//! applying dozens of steps. The pin ports only the interchange-critical subset (datasource-string→ref,
//! panel-type renames) and applies it unconditionally to any v1 export. Anything the pin does **not**
//! port degrades with a version notice (`MigrateReport.degraded`) rather than silently running the
//! full chain — the honest bound the P3 scope pins. Each step is one file (FILE-LAYOUT).

mod datasource_ref;
mod panel_type;

use serde_json::Value;

/// The current schema version the pin normalizes *toward*. Exports at or above the ported floor still
/// only get the ported steps; the number is for the report, not a full-chain target.
pub const PINNED_SCHEMA_VERSION: u64 = 33;

/// What the migration did — for the conversion report so nothing is silently dropped.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MigrateReport {
    /// The export's original `schemaVersion`.
    pub from_version: u64,
    /// Human-readable names of the ported steps that ran.
    pub applied: Vec<String>,
    /// A notice when the export predates or exceeds the ported subset — the caller should surface it.
    pub degraded: Option<String>,
}

/// Apply the ported migration subset to a detected v1 dashboard in place.
pub fn migrate_v1(root: &mut Value, from_version: u64) -> MigrateReport {
    let mut report = MigrateReport {
        from_version,
        ..Default::default()
    };

    datasource_ref::migrate(root);
    report
        .applied
        .push("datasource-string→ref (v33)".to_string());

    panel_type::migrate(root);
    report
        .applied
        .push("panel-type renames (graph/singlestat)".to_string());

    // The ported subset is a floor, not the full chain: an export older than a hand-verified version
    // may rely on a migration step we didn't port. Note it — never run the un-ported chain silently.
    if from_version > 0 && from_version < 21 {
        report.degraded = Some(format!(
            "schemaVersion {from_version} predates the ported migration subset (floor ~v21); \
             only datasource-ref + panel-type renames were applied — verify the result"
        ));
    } else if from_version == 0 {
        report.degraded =
            Some("export had no schemaVersion; applied the ported subset blind".to_string());
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ported_subset_applies_both_steps() {
        let mut root = json!({
            "schemaVersion": 30,
            "panels": [{"type": "graph", "datasource": "Prometheus"}]
        });
        let report = migrate_v1(&mut root, 30);
        assert_eq!(root["panels"][0]["type"], json!("timeseries"));
        assert_eq!(
            root["panels"][0]["datasource"],
            json!({"uid": "Prometheus"})
        );
        assert_eq!(report.applied.len(), 2);
        assert!(report.degraded.is_none());
    }

    #[test]
    fn ancient_version_degrades_with_notice() {
        let mut root = json!({"schemaVersion": 12, "panels": []});
        let report = migrate_v1(&mut root, 12);
        assert!(report.degraded.is_some());
    }

    #[test]
    fn missing_version_degrades_blind() {
        let mut root = json!({"panels": []});
        let report = migrate_v1(&mut root, 0);
        assert!(report.degraded.unwrap().contains("no schemaVersion"));
    }
}
