//! `federation.migrate {source, schema, dry_run?}` → `{statements, applied, destructive_refusal?}`
//! (schema-designer scope). The host:
//!   1. authorizes `mcp:federation.migrate:call` (admin-only — applying DDL is the destructive
//!      authority; workspace-first);
//!   2. resolves `{source}` + `net:*` + mediates the DSN (the same gated pipeline as `federation
//!      .query`);
//!   3. routes the designed schema to the supervised sidecar, which diffs it against the live
//!      catalog and (when `dry_run: false`) applies the additive DDL in one transaction.
//!
//! **The Ask gate (scope Risk 3, open-question lean #5):** `dry_run: true` is the DEFAULT — the
//! verb returns the planned statements without touching the source. The UI's "Apply" button sends
//! `dry_run: false` as the explicit second step. A migrate is never silent: the admin cap +
//! the explicit flag + the dispatch audit (`emit_dispatch_decision` records every call) together
//! form the audit trail. *(Rejected: reusing `agent.decide`'s settle path — too agent-loop-shaped
//! for a human-in-the-UI confirm; rejected the held-effect reactor — too heavy.)*

use lb_auth::Principal;
use lb_supervisor::Launcher;
use serde_json::{json, Value};

use super::authorize::authorize;
use super::error::FederationError;
use super::net::{enforce_endpoint, FEDERATION_EXT};
use super::record::resolve;
use super::secret::mediate_dsn;
use crate::boot::Node;

/// Plan + (optionally) apply a migrate of `schema` against `source` in `ws` as `caller`. `dry_run`
/// defaults to true (the Ask gate — nothing applies unless the caller explicitly opts in). Returns
/// the sidecar's `{statements, applied, destructive_refusal?}` JSON. The DSN is never returned.
pub async fn federation_migrate<L: Launcher>(
    node: &Node,
    launcher: &L,
    caller: &Principal,
    ws: &str,
    source: &str,
    schema: &Value,
    dry_run: bool,
    ts: u64,
) -> Result<Value, FederationError> {
    authorize(caller, ws, "federation.migrate")?;

    let ds = resolve(&node.store, ws, source)
        .await?
        .ok_or(FederationError::NotFound)?;
    enforce_endpoint(&node.store, ws, &ds.endpoint).await?;
    let dsn = mediate_dsn(node, ws, &ds.secret_ref).await?;

    let input = json!({
        "kind": ds.kind,
        "dsn": dsn,
        "source": source,
        "schema": schema,
        "dry_run": dry_run,
    })
    .to_string();

    let out = crate::native::call_sidecar(
        node,
        launcher,
        caller,
        ws,
        FEDERATION_EXT,
        "federation.migrate",
        &input,
        ts,
    )
    .await
    .map_err(|e| FederationError::Sidecar(e.to_string()))?;

    serde_json::from_str(&out).map_err(|e| FederationError::Sidecar(e.to_string()))
}

/// The palette/agent descriptor for `federation.migrate`. `dry_run` defaults to true (the Ask
/// gate); an agent proposing a migrate must name the schema + source and opt into apply explicitly.
pub fn migrate_descriptor() -> lb_mcp::ToolDescriptor {
    lb_mcp::ToolDescriptor {
        name: "federation.migrate".to_string(),
        title: "Plan/apply a designed schema to a datasource (additive DDL, dry-run default)"
            .to_string(),
        group: "federation".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "source": { "type": "string", "x-lb": { "entity": "datasource" } },
                "schema": {
                    "type": "object",
                    "description": "the designed schema (a db_schema record's {tables, fks})",
                    "x-lb": { "entity": "dbschema" }
                },
                "dry_run": {
                    "type": "boolean",
                    "default": true,
                    "description": "default true — plan only; false applies (admin, the Ask gate)"
                }
            },
            "required": ["source", "schema"]
        })),
        result: None,
    }
}
