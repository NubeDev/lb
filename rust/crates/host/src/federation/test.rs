//! `datasource.test {source}` — a real connectivity probe (datasources scope). Gated
//! `mcp:datasource.test:call` (workspace-first). Resolves the source in the caller's workspace,
//! enforces `net:*`, mediates the DSN, and asks the sidecar to open a live connection — green/red for
//! the UI. Like `federation.query` it never returns the DSN.

use lb_auth::Principal;
use lb_supervisor::Launcher;
use serde_json::{json, Value};

use super::authorize::authorize;
use super::error::FederationError;
use super::net::{enforce_endpoint, FEDERATION_EXT};
use super::record::resolve;
use super::secret::mediate_dsn;
use crate::boot::Node;

/// Probe `source` in `ws` as `caller`. Returns `{ok: true}` on a live connection; a connection
/// failure surfaces as a sidecar error (red).
pub async fn datasource_test<L: Launcher>(
    node: &Node,
    launcher: &L,
    caller: &Principal,
    ws: &str,
    source: &str,
    ts: u64,
) -> Result<Value, FederationError> {
    authorize(caller, ws, "datasource.test")?;

    let ds = resolve(&node.store, ws, source)
        .await?
        .ok_or(FederationError::NotFound)?;
    enforce_endpoint(&node.store, ws, &ds.endpoint).await?;
    let dsn = mediate_dsn(node, ws, &ds.secret_ref).await?;

    let input = json!({ "kind": ds.kind, "dsn": dsn }).to_string();
    let out = crate::native::call_sidecar(
        node,
        launcher,
        caller,
        ws,
        FEDERATION_EXT,
        "datasource.test",
        &input,
        ts,
    )
    .await
    .map_err(|e| FederationError::Sidecar(e.to_string()))?;

    serde_json::from_str(&out).map_err(|e| FederationError::Sidecar(e.to_string()))
}
