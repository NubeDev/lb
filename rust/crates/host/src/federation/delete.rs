//! `federation.delete {source, table, key, rows}` → `{affected}` — the bounded, structured row
//! DELETE verb (entity-binding scope, O-2). It mirrors `federation.write` EXACTLY in caps and
//! shape; it is NOT a tombstone hack. The host:
//!   1. authorizes `mcp:federation.delete:call` (workspace-first — the deny path);
//!   2. resolves `{source}` to a `datasource:{ws}:{name}` record IN THE CALLER'S WORKSPACE
//!      (un-spoofable — a caller cannot name a cross-tenant source);
//!   3. enforces `net:*` for the source's endpoint against the federation install grant;
//!   4. mediates the DSN out of `lb-secrets` under the federation extension's OWN grant;
//!   5. routes the structured key values to the supervised sidecar, which generates the
//!      parameterized `DELETE ... WHERE <key>=?` and runs it through `Source::delete_rows`.
//!
//! Row-capped (1000) at the sidecar. The caller NEVER supplies SQL — only structured key columns
//! and key-aligned value rows — so there is no injection surface at this layer (the sidecar
//! re-validates identifiers). SurrealDB is never deleted from via this path (rule 2): this verb only
//! reaches a registered EXTERNAL source.

use lb_auth::Principal;
use lb_supervisor::Launcher;
use serde_json::{json, Value};

use super::authorize::authorize;
use super::error::FederationError;
use super::net::{enforce_endpoint, FEDERATION_EXT};
use super::record::resolve;
use super::secret::mediate_dsn;
use crate::boot::Node;

/// Run a bounded delete against the registered `source` in `ws` as `caller`. `key` names the
/// identifying columns; `rows[i]` is a key-aligned array of values, so one row deletes every DB row
/// matching (`key` columns = the given values). Returns the sidecar's `{affected}` JSON value. The
/// DSN is mediated host-side and never returned.
#[allow(clippy::too_many_arguments)]
pub async fn federation_delete<L: Launcher>(
    node: &Node,
    launcher: &L,
    caller: &Principal,
    ws: &str,
    source: &str,
    table: &str,
    key: &[String],
    rows: &[Value],
    ts: u64,
) -> Result<Value, FederationError> {
    authorize(caller, ws, "federation.delete")?;

    let ds = resolve(&node.store, ws, source)
        .await?
        .ok_or(FederationError::NotFound)?;
    enforce_endpoint(&node.store, ws, &ds.endpoint).await?;
    let dsn = mediate_dsn(node, ws, &ds.secret_ref).await?;

    let input = json!({
        "kind": ds.kind,
        "dsn": dsn,
        "source": source,
        "table": table,
        "key": key,
        "rows": rows,
    })
    .to_string();

    let out = crate::native::call_sidecar(
        node,
        launcher,
        caller,
        ws,
        FEDERATION_EXT,
        "federation.delete",
        &input,
        ts,
    )
    .await
    .map_err(|e| FederationError::Sidecar(e.to_string()))?;

    serde_json::from_str(&out).map_err(|e| FederationError::Sidecar(e.to_string()))
}

/// The palette/agent descriptor for `federation.delete` — `{source, table, key, rows}`.
/// `x-lb-entity: datasource` drives the same `@`-picker as `federation.write`'s `source`. `key`
/// names the identifying columns; each `rows[i]` is a key-aligned array of values.
pub fn delete_descriptor() -> lb_mcp::ToolDescriptor {
    lb_mcp::ToolDescriptor {
        emits_external: false,
        name: "federation.delete".to_string(),
        title: "Delete rows from a registered datasource (bounded, structured key match)"
            .to_string(),
        group: "federation".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "source": { "type": "string", "x-lb": { "entity": "datasource" } },
                "table": { "type": "string", "x-lb": { "entity": "dbschema-table" } },
                "key": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "the identifying columns to match on"
                },
                "rows": {
                    "type": "array",
                    "description": "each row is a key-aligned array of values to match"
                }
            },
            "required": ["source", "table", "key", "rows"]
        })),
        result: None,
    }
}
