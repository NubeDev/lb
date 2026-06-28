//! `datasource.list {}` — the registered sources in this workspace, NO secrets in the output
//! (datasources scope). Gated `mcp:datasource.list:call` (workspace-first). Returns name + kind +
//! endpoint + the secret REF (a pointer, never the DSN value) — a tombstoned source is omitted.

use lb_auth::Principal;
use lb_store::list as store_list;

use super::authorize::authorize;
use super::error::FederationError;
use super::record::{datasource_tag, Datasource, TABLE};
use crate::boot::Node;

/// A list-row for a registered datasource — deliberately omits any secret VALUE (only the ref).
#[derive(Debug, Clone, serde::Serialize)]
pub struct DatasourceSummary {
    pub name: String,
    pub kind: String,
    pub endpoint: String,
    pub secret_ref: String,
}

/// List the (non-removed) datasources registered in `ws`.
pub async fn datasource_list(
    node: &Node,
    caller: &Principal,
    ws: &str,
) -> Result<Vec<DatasourceSummary>, FederationError> {
    authorize(caller, ws, "datasource.list")?;

    let rows = store_list(&node.store, ws, TABLE, "tag", &datasource_tag()).await?;
    let mut out = Vec::new();
    for value in rows {
        let ds: Datasource = serde_json::from_value(value)
            .map_err(|e| lb_store::StoreError::Decode(e.to_string()))?;
        if ds.removed {
            continue;
        }
        out.push(DatasourceSummary {
            name: ds.name,
            kind: ds.kind,
            endpoint: ds.endpoint,
            secret_ref: ds.secret_ref,
        });
    }
    Ok(out)
}
