//! `federation.write {source, table, columns, rows, key?}` → `{affected}` — the bounded data-write
//! verb (schema-designer scope). The host:
//!   1. authorizes `mcp:federation.write:call` (workspace-first — the deny path);
//!   2. resolves `{source}` to a `datasource:{ws}:{name}` record IN THE CALLER'S WORKSPACE
//!      (un-spoofable — a caller cannot name a cross-tenant source);
//!   3. enforces `net:*` for the source's endpoint against the federation install grant;
//!   4. mediates the DSN out of `lb-secrets` under the federation extension's OWN grant;
//!   5. routes the structured rows to the supervised sidecar, which generates the parameterized
//!      INSERT/UPSERT and runs it through `Source::write_rows`.
//!
//! Row-capped (1000) at the sidecar; past the cap the answer is "use federation.export" (scope:
//! "Bulk is a job; per-message is a verb", §6.1). The caller NEVER supplies SQL — only structured
//! rows — so there is no injection surface at this layer (the sidecar re-validates identifiers).
//! SurrealDB is never written (rule 2): this verb only reaches a registered EXTERNAL source.

use lb_auth::Principal;
use lb_supervisor::Launcher;
use serde_json::{json, Value};

use super::authorize::authorize;
use super::error::FederationError;
use super::net::{enforce_endpoint, FEDERATION_EXT};
use super::record::resolve;
use super::secret::mediate_dsn;
use crate::boot::Node;

/// Run a bounded write against the registered `source` in `ws` as `caller`. `columns` names the
/// column order; `rows[i]` is a column-aligned array. `key` (optional) names the conflict columns
/// for an idempotent UPSERT (redelivery writes the same row once). Returns the sidecar's
/// `{affected}` JSON value. The DSN is mediated host-side and never returned.
pub async fn federation_write<L: Launcher>(
    node: &Node,
    launcher: &L,
    caller: &Principal,
    ws: &str,
    source: &str,
    table: &str,
    columns: &[String],
    rows: &[Value],
    key: Option<&[String]>,
    ts: u64,
) -> Result<Value, FederationError> {
    authorize(caller, ws, "federation.write")?;

    let ds = resolve(&node.store, ws, source)
        .await?
        .ok_or(FederationError::NotFound)?;
    enforce_endpoint(&node.store, ws, &ds.endpoint).await?;
    let dsn = mediate_dsn(node, ws, &ds.secret_ref).await?;

    let mut input = json!({
        "kind": ds.kind,
        "dsn": dsn,
        "source": source,
        "table": table,
        "columns": columns,
        "rows": rows,
    });
    if let Some(key) = key {
        input["key"] = json!(key);
    }
    let input = input.to_string();

    let out = crate::native::call_sidecar(
        node,
        launcher,
        caller,
        ws,
        FEDERATION_EXT,
        "federation.write",
        &input,
        ts,
    )
    .await
    .map_err(|e| FederationError::Sidecar(e.to_string()))?;

    serde_json::from_str(&out).map_err(|e| FederationError::Sidecar(e.to_string()))
}

/// The palette/agent descriptor for `federation.write` — `{source, table, columns, rows, key?}`.
/// `x-lb-entity: datasource` drives the same `@`-picker as `federation.query`'s `source`. The flow
/// `tool` node's config form renders `table`/`columns` from the matching `db_schema` record.
pub fn write_descriptor() -> lb_mcp::ToolDescriptor {
    lb_mcp::ToolDescriptor {
        name: "federation.write".to_string(),
        title: "Write rows to a registered datasource (bounded INSERT/UPSERT)".to_string(),
        group: "federation".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "source": { "type": "string", "x-lb": { "entity": "datasource" } },
                "table": { "type": "string", "x-lb": { "entity": "dbschema-table" } },
                "columns": { "type": "array", "items": { "type": "string" } },
                "rows": {
                    "type": "array",
                    "description": "each row is a column-aligned array of values"
                },
                "key": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "conflict columns for an idempotent UPSERT (optional)"
                }
            },
            "required": ["source", "table", "columns", "rows"]
        })),
        result: None,
    }
}
