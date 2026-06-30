//! `datasource.add {name, kind, endpoint, secret_ref?, dsn?}` (admin) — register a federated source
//! (datasources scope). Admin-gated (`mcp:datasource.add:call`, workspace-first). The DSN, if
//! supplied, is written to `lb-secrets` (NOT into the datasource record); only the secret REF lands
//! in the `datasource:{ws}:{name}` record. The record never holds the connection string (§6.7).
//!
//! The `net:*` approval is the admin's install-time grant to the federation extension (enforced
//! pre-connect, `net.rs`); registration here records the endpoint that grant gates.

use lb_auth::Principal;
use lb_secrets::{set_with as secret_set_with, Visibility};

use super::authorize::authorize;
use super::error::FederationError;
use super::record::{put, Datasource};
use crate::boot::Node;

/// Register `name` as a `kind` source at `endpoint` in `ws`. `secret_ref` defaults to
/// `federation/{name}`. If `dsn` is supplied, it is stored into `lb-secrets` at that ref under the
/// admin's grant (the admin must hold `secret:federation/*:write`) — the record stores only the ref.
#[allow(clippy::too_many_arguments)]
pub async fn datasource_add(
    node: &Node,
    caller: &Principal,
    ws: &str,
    name: &str,
    kind: &str,
    endpoint: &str,
    secret_ref: Option<&str>,
    dsn: Option<&str>,
    ts: u64,
) -> Result<(), FederationError> {
    authorize(caller, ws, "datasource.add")?;

    let secret_ref = secret_ref
        .map(str::to_string)
        .unwrap_or_else(|| format!("federation/{name}"));

    // If the admin handed the DSN, mediate it into the secret store now (under the admin's
    // authority) — the value never returns and never reaches the datasource record. The secret is
    // `Workspace`-visible because the mediated pool runs as a DIFFERENT principal (`ext:federation`,
    // see `secret::mediate_dsn`) than the admin who registered it; gate 3's owner wall would
    // otherwise deny the pool. The capability grant (`secret:federation/*:get` on the install) is
    // still required, so this is "shared with the workspace" not "public to the world".
    if let Some(dsn) = dsn {
        secret_set_with(
            &node.store,
            caller,
            ws,
            &secret_ref,
            dsn,
            Visibility::Workspace,
        )
        .await
        .map_err(|_| FederationError::Denied)?;
    }

    let ds = Datasource::new(name, kind, endpoint, secret_ref, ts);
    put(&node.store, ws, &ds).await?;
    Ok(())
}
