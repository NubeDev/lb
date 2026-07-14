//! Panel-type renames — the ported subset of Grafana's `DashboardMigrator` type migrations.
//!
//! Old panel types were renamed to the current ones across several schema versions. The pin ports
//! the three that matter for interchange: `graph` → `timeseries` (v26+ angular graph → the react
//! panel), `singlestat`/`grafana-singlestat-panel` → `stat`, and the gauge-mode singlestat → `gauge`.
//! The rename is `type`-only — the full `DashboardMigrator` also rewrites each panel's options/
//! fieldConfig, which the lb-viz/mapper side owns; here we only fix the discriminator so the mapper
//! picks the right `view`. An unknown/newer type is left verbatim (carried-opaque + reported upstream).

use serde_json::Value;

/// Rewrite legacy panel `type` values in place, recursing into nested `panels` (collapsed rows).
pub fn migrate(root: &mut Value) {
    if let Some(panels) = root.get_mut("panels").and_then(Value::as_array_mut) {
        for p in panels.iter_mut() {
            rename(p);
            // Rows nest their panels.
            migrate(p);
        }
    }
}

fn rename(panel: &mut Value) {
    let Some(obj) = panel.as_object_mut() else {
        return;
    };
    let Some(ty) = obj.get("type").and_then(Value::as_str) else {
        return;
    };
    let new_ty = match ty {
        "graph" => "timeseries",
        // singlestat in gauge mode becomes `gauge`; otherwise `stat`.
        "singlestat" | "grafana-singlestat-panel" => {
            if is_gauge_mode(obj) {
                "gauge"
            } else {
                "stat"
            }
        }
        _ => return,
    };
    obj.insert("type".to_string(), Value::String(new_ty.to_string()));
}

/// A classic singlestat rendered as a gauge when `gauge.show == true`.
fn is_gauge_mode(obj: &serde_json::Map<String, Value>) -> bool {
    obj.get("gauge")
        .and_then(|g| g.get("show"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn graph_becomes_timeseries() {
        let mut root = json!({"panels": [{"type": "graph"}]});
        migrate(&mut root);
        assert_eq!(root["panels"][0]["type"], json!("timeseries"));
    }

    #[test]
    fn singlestat_becomes_stat() {
        let mut root = json!({"panels": [{"type": "singlestat"}]});
        migrate(&mut root);
        assert_eq!(root["panels"][0]["type"], json!("stat"));
    }

    #[test]
    fn singlestat_gauge_mode_becomes_gauge() {
        let mut root = json!({"panels": [{"type": "singlestat", "gauge": {"show": true}}]});
        migrate(&mut root);
        assert_eq!(root["panels"][0]["type"], json!("gauge"));
    }

    #[test]
    fn nested_row_panels_renamed() {
        let mut root = json!({"panels": [{"type": "row", "panels": [{"type": "graph"}]}]});
        migrate(&mut root);
        assert_eq!(root["panels"][0]["panels"][0]["type"], json!("timeseries"));
    }

    #[test]
    fn unknown_type_left_verbatim() {
        let mut root = json!({"panels": [{"type": "flamegraph"}]});
        migrate(&mut root);
        assert_eq!(root["panels"][0]["type"], json!("flamegraph"));
    }
}
