//! `datasource.add {name, kind, endpoint, secret_ref?, dsn?}` (admin) — register a federated source
//! (datasources scope). Admin-gated (`mcp:datasource.add:call`, workspace-first). The DSN, if
//! supplied, is written to `lb-secrets` (NOT into the datasource record); only the secret REF lands
//! in the `datasource:{ws}:{name}` record. The record never holds the connection string (§6.7).
//!
//! The `net:*` approval is the admin's install-time grant to the federation extension (enforced
//! pre-connect, `net.rs`); registration here records the endpoint that grant gates.

use lb_auth::Principal;

use super::authorize::authorize;
use super::error::FederationError;
use super::net::grant_endpoint;
use super::record::{put, Datasource};
use super::secret::store_dsn;
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

    // If the admin handed the DSN, mediate it into the secret store now — OWNED by the stable
    // `ext:federation` principal, not this (varying) admin caller. That single-owner invariant is
    // what makes CRUD collision-free: a later admin (a different login, the boot seed, a future IdP
    // user) can overwrite or remove the source without hitting the secrets owner wall (gate 3),
    // because the owner is always the extension, never whoever ran `add`. The value never returns
    // and never reaches the datasource record (§6.7). See `secret::store_dsn`.
    if let Some(dsn) = dsn {
        store_dsn(node, ws, &secret_ref, dsn).await?;
    }

    // Self-approve the endpoint: append `net:tls:{host}:{port}:connect` to the federation install
    // grant so a source added from the UI connects with NO boot env var / node restart. Registration
    // (already admin-gated above) IS the endpoint approval — the `net:*` wall stays enforced pre-connect
    // (`net.rs::enforce_endpoint`), this just records what the admin approved by adding the source.
    // Idempotent (a no-op when a grant already covers the endpoint).
    grant_endpoint(&node.store, ws, endpoint).await?;

    let ds = Datasource::new(name, kind, endpoint, secret_ref, ts);
    put(&node.store, ws, &ds).await?;
    Ok(())
}
