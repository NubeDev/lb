//! `federation.schema {source, table?}` → the native discovery verb (datasources-ux scope). Lists a
//! source's user tables when `table` is absent, or one table's columns when present. It is the
//! NO-SQL "browse the source" path: the UI never writes catalog SQL (the engine only registers the
//! tables a query references, so an `information_schema` SELECT is unreachable); discovery reads the
//! source's real catalog through the sidecar.
//!
//! It reuses `federation.query`'s exact gated pipeline — resolve the source IN THE CALLER'S
//! WORKSPACE, enforce `net:*`, mediate the DSN, route to the supervised sidecar — and authorizes
//! under the SAME read cap (`mcp:federation.query:call`): discovery is the same read privilege as a
//! live query, so no new capability/grant is introduced. SurrealDB is never a source (rule 2).

use lb_auth::Principal;
use lb_supervisor::Launcher;
use serde_json::{json, Value};

use super::authorize::authorize;
use super::error::FederationError;
use super::net::{enforce_endpoint, FEDERATION_EXT};
use super::record::resolve;
use super::secret::mediate_dsn;
use crate::boot::Node;

/// Discover the schema of `source` in `ws` as `caller`. With `table = None` the sidecar lists user
/// tables; with `table = Some(t)` it returns `t`'s columns. Returns the sidecar's JSON value
/// (`{tables:[…]}` / `{columns:[…]}`). The DSN is mediated host-side and never returned.
pub async fn federation_schema<L: Launcher>(
    node: &Node,
    launcher: &L,
    caller: &Principal,
    ws: &str,
    source: &str,
    table: Option<&str>,
    ts: u64,
) -> Result<Value, FederationError> {
    // Discovery is the same read privilege as a live query — authorize under the read cap so no new
    // capability grant is needed (the dev/admin roles already carry `mcp:federation.query:call`).
    authorize(caller, ws, "federation.query")?;

    // Resolve the alias to a registered source IN THIS workspace — un-spoofable (the wall).
    let ds = resolve(&node.store, ws, source)
        .await?
        .ok_or(FederationError::NotFound)?;

    // `net:*` — refuse, opaque, if the source's endpoint is not in the admin-approved grant.
    enforce_endpoint(&node.store, ws, &ds.endpoint).await?;

    // Mediate the DSN under the FEDERATION extension's own grant (never the caller's).
    let dsn = mediate_dsn(node, ws, &ds.secret_ref).await?;

    let mut input = json!({ "kind": ds.kind, "dsn": dsn, "source": source });
    if let Some(t) = table {
        input["table"] = json!(t);
    }
    let input = input.to_string();

    let out = crate::native::call_sidecar(
        node,
        launcher,
        caller,
        ws,
        FEDERATION_EXT,
        "federation.schema",
        &input,
        ts,
    )
    .await
    .map_err(|e| FederationError::Sidecar(e.to_string()))?;

    serde_json::from_str(&out).map_err(|e| FederationError::Sidecar(e.to_string()))
}

/// The palette/agent descriptor for `federation.schema` — a real arg schema (`{source, table?}`),
/// so a model advertised the tool can FORM a valid call (a name-only row leaves it guessing arg
/// names; the live agent probed `information_schema` SQL instead). `x-lb-entity: datasource` drives
/// the same `@`-picker as `federation.query`'s `source`.
pub fn schema_descriptor() -> lb_mcp::ToolDescriptor {
    lb_mcp::ToolDescriptor {
        name: "federation.schema".to_string(),
        title: "List a registered datasource's tables, or one table's columns".to_string(),
        group: "federation".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "source": { "type": "string", "x-lb": { "entity": "datasource" } },
                "table": { "type": "string" }
            },
            "required": ["source"]
        })),
        result: None,
    }
}
