//! Which Grafana dashboard shape is this JSON? — the v1/v2 discriminator that gates the whole pin.
//!
//! v1 is the classic model keyed by numeric `schemaVersion` (…42 in 13.2). v2 is the new
//! app-platform kind: a `dashboard.grafana.app/*` `apiVersion` OR the `elements`/`layout` shape.
//! We accept v1 (migrate + resolve below) and **reject v2 with a pointer** — the mapper has no v2
//! path, and silently treating v2 fields as v1 would drop the whole layout. Snapshots reject too.

use serde_json::Value;

/// The detected interchange shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shape {
    /// Classic dashboard, `schemaVersion` = N. Migrate to the pinned target, then map.
    V1 { schema_version: u64 },
    /// The v2 app-platform kind (`elements`/`layout` or `dashboard.grafana.app`). Rejected.
    V2,
    /// A `snapshot`-wrapped export. Rejected — not a plain dashboard.
    Snapshot,
}

/// Classify a top-level Grafana export object. Missing `schemaVersion` on an otherwise-v1-looking
/// object is treated as `V1 { schema_version: 0 }` so the migration floor degrades it with a notice
/// rather than guessing — matching Grafana's own "unknown old version" handling.
pub fn detect(root: &Value) -> Shape {
    // v2 by explicit apiVersion namespace.
    if let Some(api) = root.get("apiVersion").and_then(Value::as_str) {
        if api.starts_with("dashboard.grafana.app/") {
            return Shape::V2;
        }
    }
    // v2 by shape: the new kind carries `elements` + `layout` where v1 carries `panels`/`rows`.
    if root.get("elements").is_some() && root.get("layout").is_some() {
        return Shape::V2;
    }
    // A snapshot export nests the dashboard under `snapshot`/`dashboard` with snapshot data.
    if root.get("snapshot").is_some() {
        return Shape::Snapshot;
    }
    let schema_version = root
        .get("schemaVersion")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    Shape::V1 { schema_version }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn classic_schema_version_is_v1() {
        assert_eq!(
            detect(&json!({"schemaVersion": 30, "panels": []})),
            Shape::V1 { schema_version: 30 }
        );
    }

    #[test]
    fn app_platform_apiversion_is_v2() {
        assert_eq!(
            detect(&json!({"apiVersion": "dashboard.grafana.app/v2beta1", "spec": {}})),
            Shape::V2
        );
    }

    #[test]
    fn elements_layout_shape_is_v2() {
        assert_eq!(
            detect(&json!({"elements": {}, "layout": {"kind": "GridLayout"}})),
            Shape::V2
        );
    }

    #[test]
    fn snapshot_rejected() {
        assert_eq!(
            detect(&json!({"snapshot": {"key": "abc"}})),
            Shape::Snapshot
        );
    }

    #[test]
    fn missing_schema_version_floors_to_zero() {
        assert_eq!(
            detect(&json!({"panels": []})),
            Shape::V1 { schema_version: 0 }
        );
    }
}
