//! `dbschema.list {}` (member) — list the workspace's designed-schema records (schema-designer
//! scope). Member-gated under the read wildcard (`mcp:dbschema.list:call`, workspace-first).
//! Returns name + table count per schema — NO layout/geometry (a browse row, not a full record;
//! the canvas loads the full record via `dbschema.get` on open). A tombstoned schema is omitted.

use lb_auth::Principal;
use serde_json::{json, Value};

use super::authorize::authorize;
use super::dbschema_record::list_summaries;
use super::error::FederationError;
use crate::boot::Node;

/// List the (non-removed) `db_schema` records in `ws` — `{schemas: [{name, table_count, version}]}`.
pub async fn dbschema_list(
    node: &Node,
    caller: &Principal,
    ws: &str,
) -> Result<Value, FederationError> {
    authorize(caller, ws, "dbschema.list")?;
    let summaries = list_summaries(&node.store, ws).await?;
    Ok(json!({ "schemas": summaries }))
}
