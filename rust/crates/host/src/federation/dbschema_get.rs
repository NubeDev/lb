//! `dbschema.get {name}` (member) — read one designed-schema record (schema-designer scope).
//! Member-gated under the read wildcard (`mcp:dbschema.get:call`, workspace-first). Returns the
//! full record (`{name, version, tables, fks, layout}`) so the canvas rehydrates verbatim —
//! geometry included. A cross-tenant name resolves to nothing (the wall is structural).

use lb_auth::Principal;
use serde_json::Value;

use super::authorize::authorize;
use super::dbschema_record::resolve;
use super::error::FederationError;
use crate::boot::Node;

/// Read the `db_schema:{ws}:{name}` record. `Ok(None)` if absent (or tombstoned, or cross-tenant —
/// all read as "not here"). The caller decides whether `None` is a 404 or an empty-canvas seed.
pub async fn dbschema_get(
    node: &Node,
    caller: &Principal,
    ws: &str,
    name: &str,
) -> Result<Option<Value>, FederationError> {
    authorize(caller, ws, "dbschema.get")?;
    let rec = resolve(&node.store, ws, name).await?;
    match rec {
        None => Ok(None),
        Some(rec) => Ok(Some(
            serde_json::to_value(&rec).map_err(|e| FederationError::Sidecar(e.to_string()))?,
        )),
    }
}
