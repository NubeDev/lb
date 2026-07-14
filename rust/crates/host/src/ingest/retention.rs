//! `series.retention.*` — the capability-gated administration of series retention (series-retention
//! scope, issue #58). Three verbs, each its own MCP surface + cap:
//!   - `series.retention.set` — upsert the policy for a series-name prefix;
//!   - `series.retention.list` — the workspace's policies;
//!   - `series.retention.gc` — run one rollup-then-evict pass now (`now_ms` is the caller's logical
//!     clock; the HTTP/MCP layer stamps wall-clock when the caller omits it).
//!
//! Namespace-scoped like every series verb (the hard wall); a denial is opaque.

use lb_auth::Principal;
use lb_ingest::{delete_policy, list_policies, run_gc, set_policy, GcPass, Policy};
use lb_store::Store;

use super::authorize::authorize_ingest;
use super::error::IngestError;

/// Upsert `policy` in `ws`. Gated by `mcp:series.retention.set:call`.
pub async fn series_retention_set(
    store: &Store,
    principal: &Principal,
    ws: &str,
    policy: &Policy,
) -> Result<(), IngestError> {
    authorize_ingest(principal, ws, "series.retention.set")?;
    Ok(set_policy(store, ws, policy).await?)
}

/// The workspace's retention policies. Gated by `mcp:series.retention.list:call`.
pub async fn series_retention_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Vec<Policy>, IngestError> {
    authorize_ingest(principal, ws, "series.retention.list")?;
    Ok(list_policies(store, ws).await?)
}

/// Delete the policy at `prefix` (covered series revert to keep-forever). Gated by
/// `mcp:series.retention.set:call` — deleting a policy is the same administrative privilege as
/// setting one; no separate cap is minted.
pub async fn series_retention_delete(
    store: &Store,
    principal: &Principal,
    ws: &str,
    prefix: &str,
) -> Result<(), IngestError> {
    authorize_ingest(principal, ws, "series.retention.set")?;
    Ok(delete_policy(store, ws, prefix).await?)
}

/// Run one retention GC pass at logical time `now_ms`. Gated by `mcp:series.retention.gc:call`.
pub async fn series_retention_gc(
    store: &Store,
    principal: &Principal,
    ws: &str,
    now_ms: u64,
) -> Result<GcPass, IngestError> {
    authorize_ingest(principal, ws, "series.retention.gc")?;
    Ok(run_gc(store, ws, now_ms).await?)
}
