//! `bind` — turn a REMAPPED Grafana target into an EXECUTABLE one (viz import-export scope, the commit
//! half's last mapping stage). This is the step `to_cell::targets_to_sources` defers: it leaves
//! `Target.tool` empty because the concrete MCP tool is only knowable once the caller has bound the
//! target's datasource. `datasources::apply` performs that binding (rewriting each `{uid}` to one of
//! OUR workspace datasources); this file reads the bound ref and fills in the `tool` + the arg names
//! that tool actually reads.
//!
//! Why it must exist: `viz.query` SKIPS a target whose `tool` is empty (`viz/query.rs`, `targets()`),
//! and a tool whose required args are missing resolves to an honest-empty frame. So without this stage
//! an imported panel binds, validates, saves, and reports "mapped" — then renders blank forever. That
//! is the exact failure this file closes (`docs/debugging/dashboard/imported-grafana-panels-render-empty.md`).
//!
//! Rule 10: no datasource is special-cased. A bound ref names a datasource RECORD; `federation.query`
//! resolves that record's `kind`/`dsn` itself (`federation/schema.rs`), so one generic mapping covers
//! sqlite/postgres/every future kind without a branch here. The two reserved targets that are NOT
//! federation records (`native`, `series`) map to their own shipped verbs by the same table.
//!
//! Round-trip safety: the original Grafana target object is PRESERVED in `args` — `to_grafana` re-emits
//! `args` verbatim as the target, so export stays lossless. We only ADD the keys our tool reads
//! (`source`/`sql`), never remove Grafana's (`rawSql`, `format`, …). A key we would add that the target
//! already defines is left alone — the caller's value wins.

use serde_json::{Map, Value};

use crate::dashboard::model::{Cell, Target};

use super::DegradedItem;

/// The MCP tool a bound target dispatches to. A bound ref is either one of the two reserved
/// non-federation targets or the NAME of a datasource record in the caller's workspace — which
/// `federation.query` resolves (kind + dsn) on its own. Keep in step with `import::is_reserved_target`.
fn tool_for_binding(bound: &str) -> &'static str {
    match bound {
        "native" => "store.query",
        "series" => "series.read",
        // Every datasource RECORD — sqlite, postgres, … — is one federation read. No per-kind branch.
        _ => "federation.query",
    }
}

/// The SQL a Grafana target carries, under any of the spellings its datasource plugins use. Checked in
/// Grafana's own precedence: the explicit `rawSql` (SQL plugins) before `expr` (Prometheus/Loki).
fn grafana_sql(args: &Map<String, Value>) -> Option<&str> {
    ["rawSql", "rawQuery", "query", "expr"]
        .iter()
        .find_map(|k| args.get(*k).and_then(Value::as_str))
        .filter(|s| !s.trim().is_empty())
}

/// The bound datasource name from a target's (already remapped) `datasource` ref — `{ "uid": "<name>" }`
/// after `datasources::apply`. An unmapped ref still carries its ORIGINAL Grafana uid; the caller's
/// mappings are the only thing that makes a ref ours, so an unbound target is left for the caller's
/// degraded list rather than guessed at.
fn bound_name(datasource: &Value) -> Option<&str> {
    datasource
        .get("uid")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && *s != "__expr__")
}

/// Fill one target's `tool` + executable args from its bound datasource. Returns a `DegradedItem` when
/// the target is bound but carries no SQL we can hand the tool — honest, because that panel WILL render
/// empty and the report should say so rather than claim a clean import.
fn bind_target(target: &mut Target, cell_key: &str, bound: &[String]) -> Option<DegradedItem> {
    // Only bind what the caller actually mapped. An unmapped ref keeps its Grafana uid and already
    // degrades in `datasources::apply` — binding it would invent a source the caller never chose.
    let name = bound_name(&target.datasource)?;
    if !bound.iter().any(|b| b == name) {
        return None;
    }
    let tool = tool_for_binding(name);
    // A caller-supplied tool wins — this stage fills a GAP, it never overrides an explicit choice.
    if target.tool.is_empty() {
        target.tool = tool.to_string();
    }

    let Value::Object(args) = &mut target.args else {
        // No target object to read a query out of; nothing to bind, and the empty-args case is already
        // visible as an empty panel.
        return None;
    };

    let sql = grafana_sql(args).map(str::to_string);
    match sql {
        Some(sql) => {
            // ADD our arg names alongside Grafana's (which `to_grafana` re-emits). `entry` keeps a
            // caller-supplied value if the target already speaks our shape.
            args.entry("source".to_string())
                .or_insert_with(|| Value::String(name.to_string()));
            args.entry("sql".to_string())
                .or_insert_with(|| Value::String(sql));
            None
        }
        None => Some(DegradedItem {
            kind: "target".to_string(),
            cell: cell_key.to_string(),
            detail: format!(
                "target '{}' has no SQL query — bound to '{name}' but renders empty",
                target.ref_id
            ),
        }),
    }
}

/// Bind every target in `cells` that the caller mapped, in place. `bound` is the set of datasource
/// names the verb has VERIFIED against the caller's workspace (`import::verify_mappings`) — we bind ONLY
/// to those, so this stage can never widen the tenancy wall. Returns the degraded notices to fold into
/// the report.
pub fn bind_cells(cells: &mut [Cell], bound: &[String]) -> Vec<DegradedItem> {
    let mut degraded = Vec::new();
    for cell in cells.iter_mut() {
        let key = cell.i.clone();
        for target in cell.sources.iter_mut() {
            if let Some(d) = bind_target(target, &key, bound) {
                degraded.push(d);
            }
        }
    }
    degraded
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn cell_with(target: Value, datasource: Value) -> Cell {
        Cell {
            i: "0".into(),
            sources: vec![Target {
                ref_id: "A".into(),
                datasource,
                tool: String::new(),
                args: target,
                hide: false,
            }],
            ..Cell::default()
        }
    }

    #[test]
    fn binds_a_datasource_record_to_federation_query_with_our_arg_names() {
        let mut cells = [cell_with(
            json!({"refId": "A", "rawSql": "SELECT 1", "format": "time_series"}),
            json!({"uid": "demo-buildings"}),
        )];
        let degraded = bind_cells(&mut cells, &["demo-buildings".to_string()]);
        let t = &cells[0].sources[0];
        assert_eq!(t.tool, "federation.query");
        assert_eq!(t.args["source"], json!("demo-buildings"));
        assert_eq!(t.args["sql"], json!("SELECT 1"));
        // Grafana's own keys survive for the export round-trip.
        assert_eq!(t.args["rawSql"], json!("SELECT 1"));
        assert_eq!(t.args["format"], json!("time_series"));
        assert!(degraded.is_empty());
    }

    #[test]
    fn reserved_targets_map_to_their_own_verbs() {
        for (bound, want) in [("native", "store.query"), ("series", "series.read")] {
            let mut cells = [cell_with(
                json!({"refId": "A", "rawSql": "SELECT 1"}),
                json!({"uid": bound}),
            )];
            bind_cells(&mut cells, &[bound.to_string()]);
            assert_eq!(cells[0].sources[0].tool, want);
        }
    }

    #[test]
    fn an_unmapped_ref_is_left_alone() {
        // The caller never bound `P1234`, so it keeps its Grafana uid and stays unexecutable —
        // `datasources::apply` already degraded it; we must not invent a binding.
        let mut cells = [cell_with(
            json!({"refId": "A", "rawSql": "SELECT 1"}),
            json!({"type": "postgres", "uid": "P1234"}),
        )];
        let degraded = bind_cells(&mut cells, &["demo-buildings".to_string()]);
        assert_eq!(cells[0].sources[0].tool, "");
        assert!(cells[0].sources[0].args.get("source").is_none());
        assert!(degraded.is_empty());
    }

    #[test]
    fn a_bound_target_without_sql_degrades_honestly() {
        let mut cells = [cell_with(json!({"refId": "A"}), json!({"uid": "demo"}))];
        let degraded = bind_cells(&mut cells, &["demo".to_string()]);
        assert_eq!(degraded.len(), 1);
        assert_eq!(degraded[0].kind, "target");
        assert!(degraded[0].detail.contains("no SQL query"));
    }

    #[test]
    fn a_caller_supplied_tool_and_args_win() {
        let mut cells = [cell_with(
            json!({"refId": "A", "rawSql": "SELECT 1", "source": "chosen", "sql": "SELECT 2"}),
            json!({"uid": "demo"}),
        )];
        cells[0].sources[0].tool = "ext.custom".into();
        bind_cells(&mut cells, &["demo".to_string()]);
        let t = &cells[0].sources[0];
        assert_eq!(t.tool, "ext.custom");
        assert_eq!(t.args["source"], json!("chosen"));
        assert_eq!(t.args["sql"], json!("SELECT 2"));
    }

    #[test]
    fn prometheus_expr_is_read_as_the_query() {
        let mut cells = [cell_with(
            json!({"refId": "A", "expr": "up"}),
            json!({"uid": "demo"}),
        )];
        bind_cells(&mut cells, &["demo".to_string()]);
        assert_eq!(cells[0].sources[0].args["sql"], json!("up"));
    }
}
