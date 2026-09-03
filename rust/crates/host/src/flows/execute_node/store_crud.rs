//! The store-CRUD legs of the platform pack (ext-store-nodes scope): `store-read` / `store-write` /
//! `store-delete`. Each dispatches the existing generic verb (`store.query` / `store.write` /
//! `store.delete`) through [`call_tool_node`] under the caller's principal — so the existing gates
//! (`mcp:store.<verb>:call`, the per-table `store:<table>:write`, and the reserved-table wall inside
//! the mutate surface) all apply per node execution, unchanged.
//!
//! `store-read` builds a **parameterized** SELECT host-side: the table rides as a `type::table($tb)`
//! binding, every filter value / the id is a `$`-bound var, and the only spliced text is
//! identifier-checked field names plus a host-clamped integer limit — user text is NEVER spliced
//! into the SQL. It runs on the read-only session, behind the secret-plane wall (see `store_read`).
//!
//! Config-vs-payload precedence is uniform (the scope rule): an explicit config field wins; a
//! missing config field reads the incoming `payload` (`payload.id`, `payload.filter`, the payload
//! itself as `store-write`'s value) — so a wire can drive the row dynamically while the **table
//! stays pinned by the author** (config-only, and required by the schema).

use std::sync::Arc;

use lb_auth::Principal;
use serde_json::{json, Map, Value};

use crate::boot::Node;

use super::super::run_store::NodeOutcome;
use super::call_tool_node;
use super::store_read_sql::{build_read_sql, is_ident};

/// Read `field` config-first, falling back to an object `payload`'s same-named key (the uniform
/// precedence rule). `None` when neither carries it.
fn config_or_payload<'a>(
    config: &'a Value,
    inputs: &'a Map<String, Value>,
    field: &str,
) -> Option<&'a Value> {
    config
        .get(field)
        .filter(|v| !v.is_null())
        .or_else(|| inputs.get("payload")?.as_object()?.get(field))
        .filter(|v| !v.is_null())
}

/// The node's pinned table: config-only (required by the descriptor schema — a wire can never
/// re-point the table), identifier-checked before any dispatch.
fn table_of(node_type: &str, config: &Value) -> Result<String, String> {
    let table = config.get("table").and_then(|v| v.as_str()).unwrap_or("");
    if table.is_empty() {
        return Err(format!("{node_type} node missing config.table"));
    }
    if !is_ident(table) {
        return Err(format!(
            "{node_type} node: invalid table identifier: {table}"
        ));
    }
    Ok(table.to_string())
}

/// Unwrap one `store.query` row from the store's `{data, rev}` record envelope to the value the
/// author wrote (`lb_store::write` wraps under `data`). A row without the envelope passes through.
fn unwrap_row(row: &Value) -> Value {
    row.get("data").cloned().unwrap_or_else(|| row.clone())
}

/// `store-read`: build the parameterized SELECT and dispatch `store.query`. Emits
/// `{payload: {rows: [...]}}` — or `{payload: {row: <value|null>}}` for a single-`id` read.
pub(super) async fn store_read(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    config: &Value,
    inputs: &Map<String, Value>,
) -> NodeOutcome {
    let table = match table_of("store-read", config) {
        Ok(t) => t,
        Err(e) => return NodeOutcome::Err(e),
    };
    let id = config_or_payload(config, inputs, "id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let filter = config_or_payload(config, inputs, "filter")
        .and_then(|v| v.as_object())
        .cloned();
    let limit = config.get("limit").and_then(|v| v.as_u64());
    let order_by = config
        .get("order_by")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let desc = config
        .get("desc")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let (sql, vars) = match build_read_sql(
        &table,
        id.as_deref(),
        filter.as_ref(),
        limit,
        order_by.as_deref(),
        desc,
    ) {
        Ok(built) => built,
        Err(e) => return NodeOutcome::Err(format!("store-read node: {e}")),
    };
    // Composed SQL, so it needs no untrusted-text handling — but it keeps every OTHER protection
    // `store.query` carries. Dropping one was a real hole: a node configured with `table: "secret"`
    // returned the credential (`a_store_read_node_cannot_read_the_secret_plane`). Hence the cap
    // check, the secret-plane wall (the table is author CONFIG), and a read-only session.
    if let Err(e) = crate::store_query::authorize_store_query(principal, ws, "store.query") {
        return NodeOutcome::Err(format!("store-read node: {e}"));
    }
    let bindings: Vec<(String, Value)> = vars.into_iter().collect();
    if let Err(e) = crate::store_query::ensure_read_only_with_vars(&sql, &bindings) {
        return NodeOutcome::Err(format!("store-read node: {e}"));
    }
    let queried = node
        .store
        .query_ws_readonly(ws, &sql, bindings)
        .await
        .and_then(|mut resp| {
            resp.take::<Vec<Value>>(0)
                .map_err(|e| lb_store::StoreError::Decode(e.to_string()))
        });
    match queried {
        Ok(raw) => {
            let rows: Vec<Value> = raw.iter().map(unwrap_row).collect();
            let payload = if id.is_some() {
                json!({ "row": rows.first().cloned().unwrap_or(Value::Null) })
            } else {
                json!({ "rows": rows })
            };
            NodeOutcome::ok(json!({ "payload": payload }))
        }
        // The store's own error, surfaced as the node's — not swallowed into an empty row set,
        // which would read to a flow author as "the table is empty" rather than "the read failed".
        Err(e) => NodeOutcome::Err(format!("store-read node: {e}")),
    }
}

/// `store-write`: upsert `{table, id, value}` via `store.write`. `id` defaults to `payload.id`,
/// else a freshly minted ULID (the host's one id scheme); `value` defaults to the incoming payload.
/// Emits `{payload: {table, id}}` (the verb's own return) so a downstream node learns the key.
pub(super) async fn store_write(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    config: &Value,
    inputs: &Map<String, Value>,
) -> NodeOutcome {
    let table = match table_of("store-write", config) {
        Ok(t) => t,
        Err(e) => return NodeOutcome::Err(e),
    };
    let id = config_or_payload(config, inputs, "id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(lb_store::new_ulid);
    let value = match config.get("value").filter(|v| !v.is_null()) {
        Some(v) => v.clone(),
        None => match inputs.get("payload").filter(|v| !v.is_null()) {
            Some(p) => p.clone(),
            None => {
                return NodeOutcome::Err(
                    "store-write node has no value (config.value and payload both absent)".into(),
                )
            }
        },
    };
    let args = json!({ "table": table, "id": id, "value": value });
    match call_tool_node(node, principal, ws, "store.write", &args).await {
        NodeOutcome::Ok { emitted, .. } => NodeOutcome::ok(json!({ "payload": emitted })),
        other => other,
    }
}

/// `store-delete`: erase `{table, id}` via `store.delete` (idempotent in the verb). A terminal
/// sink — the ack payload is recorded but wired nowhere (no outputs).
pub(super) async fn store_delete(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    config: &Value,
    inputs: &Map<String, Value>,
) -> NodeOutcome {
    let table = match table_of("store-delete", config) {
        Ok(t) => t,
        Err(e) => return NodeOutcome::Err(e),
    };
    let Some(id) = config_or_payload(config, inputs, "id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
    else {
        return NodeOutcome::Err(
            "store-delete node has no id (config.id and payload.id both absent)".into(),
        );
    };
    let args = json!({ "table": table, "id": id });
    match call_tool_node(node, principal, ws, "store.delete", &args).await {
        NodeOutcome::Ok { emitted, .. } => NodeOutcome::ok(json!({ "payload": emitted })),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filter(pairs: &[(&str, Value)]) -> Map<String, Value> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    /// Table-driven: id / filter / limit / order_by / desc combinations produce the expected
    /// parameterized SQL (values only ever appear as `$`-bound vars).
    #[test]
    fn read_sql_construction_table() {
        // Bare read: default limit.
        let (sql, vars) = build_read_sql("site", None, None, None, None, false).unwrap();
        assert_eq!(
            sql,
            "SELECT record::id(id) AS id, data, rev FROM type::table($tb) LIMIT 100"
        );
        assert_eq!(vars["tb"], "site");

        // Single-id read.
        let (sql, vars) = build_read_sql("site", Some("row1"), None, None, None, false).unwrap();
        assert_eq!(
            sql,
            "SELECT record::id(id) AS id, data, rev FROM type::table($tb) WHERE record::id(id) = $id LIMIT 100"
        );
        assert_eq!(vars["id"], "row1");

        // Flat filter + explicit limit + order desc.
        let f = filter(&[("region", json!("nsw")), ("active", json!(true))]);
        let (sql, vars) =
            build_read_sql("site", None, Some(&f), Some(50), Some("ts"), true).unwrap();
        assert_eq!(
            sql,
            "SELECT record::id(id) AS id, data, rev, data.ts AS _ord OMIT _ord \
             FROM type::table($tb) WHERE data.region = $f0 AND data.active = $f1 \
             ORDER BY _ord DESC LIMIT 50"
        );
        assert_eq!(vars["f0"], "nsw");
        assert_eq!(vars["f1"], true);

        // Ascending order keeps no DESC suffix; id AND filter compose.
        let f = filter(&[("kind", json!("ems"))]);
        let (sql, _) =
            build_read_sql("device", Some("d1"), Some(&f), Some(7), Some("name"), false).unwrap();
        assert_eq!(
            sql,
            "SELECT record::id(id) AS id, data, rev, data.name AS _ord OMIT _ord \
             FROM type::table($tb) WHERE record::id(id) = $id AND data.kind = $f0 \
             ORDER BY _ord LIMIT 7"
        );

        // Limit clamps to the hard max (and up to the floor).
        let (sql, _) = build_read_sql("site", None, None, Some(999_999), None, false).unwrap();
        assert!(sql.ends_with("LIMIT 1000"), "clamped: {sql}");
        let (sql, _) = build_read_sql("site", None, None, Some(0), None, false).unwrap();
        assert!(sql.ends_with("LIMIT 1"), "floored: {sql}");
    }

    /// A hostile filter VALUE (quotes / semicolons / a would-be second statement) is bound as a
    /// `$var`, never spliced — the SQL text is byte-identical regardless of the value.
    #[test]
    fn hostile_filter_values_are_bound_not_spliced() {
        let hostile = json!("x'; DELETE FROM flow; --\"");
        let f = filter(&[("name", hostile.clone())]);
        let (sql, vars) = build_read_sql("site", None, Some(&f), None, None, false).unwrap();
        assert!(
            !sql.contains("DELETE") && !sql.contains('\'') && !sql.contains(';'),
            "hostile value leaked into SQL: {sql}"
        );
        assert_eq!(sql, "SELECT record::id(id) AS id, data, rev FROM type::table($tb) WHERE data.name = $f0 LIMIT 100");
        assert_eq!(vars["f0"], hostile);

        // Same for a hostile id.
        let (sql, vars) =
            build_read_sql("site", Some("a' OR 1=1 --"), None, None, None, false).unwrap();
        assert!(!sql.contains("OR 1=1"), "hostile id leaked: {sql}");
        assert_eq!(vars["id"], "a' OR 1=1 --");
    }

    /// Invalid identifiers (table / filter field / order_by) are rejected BEFORE any dispatch.
    #[test]
    fn invalid_identifiers_rejected_before_dispatch() {
        assert!(build_read_sql("bad-table", None, None, None, None, false).is_err());
        assert!(build_read_sql("site; DROP", None, None, None, None, false).is_err());
        assert!(build_read_sql("", None, None, None, None, false).is_err());
        assert!(build_read_sql("1site", None, None, None, None, false).is_err());
        let f = filter(&[("a b", json!(1))]);
        assert!(build_read_sql("site", None, Some(&f), None, None, false).is_err());
        let f = filter(&[("a;--", json!(1))]);
        assert!(build_read_sql("site", None, Some(&f), None, None, false).is_err());
        assert!(build_read_sql("site", None, None, None, Some("ts DESC; x"), false).is_err());
        assert!(build_read_sql("site", None, None, None, Some("data.ts"), false).is_err());
        // Plain identifiers (incl. underscore) pass.
        assert!(build_read_sql("ops_heartbeat", None, None, None, Some("_ts"), true).is_ok());
    }
}
