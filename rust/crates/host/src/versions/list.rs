//! `versions.list { kind, id, limit? }` — one entity's ring as **metadata only**, newest-first.
//!
//! Never ships snapshots: a 20-deep ring of 40-cell dashboards is real bytes, and a history dialog
//! opens before it needs any of them (`versions.get` fetches the one the user selected). This is the
//! read a viewer holds — seeing history you cannot restore is correct, not a leak.

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_store::{read_versioned, Store};

use super::cap::{read_config, resolve_cap};
use super::error::VersionsError;
use super::plan::plan_for_kind;
use super::record::{snapshot_hash, VersionMeta};
use super::store::{read_ring, MAX_LIST};

/// The metadata rows plus the cap in force — a client renders "20 of 20 kept" without a second call.
pub struct VersionList {
    pub versions: Vec<VersionMeta>,
    pub cap: usize,
}

pub async fn versions_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
    kind: &str,
    id: &str,
    limit: Option<usize>,
) -> Result<VersionList, VersionsError> {
    authorize_tool(principal, ws, "versions.list").map_err(|_| VersionsError::Denied)?;
    let plan = plan_for_kind(kind).ok_or_else(|| unknown_kind(kind))?;

    let rows = read_ring(store, ws, kind, id, limit.unwrap_or(MAX_LIST)).await?;

    // The "current" marker: whichever versions have the same content as the LIVE record. Computed
    // per read against the entity itself — storing it would go stale the moment the entity changed,
    // and comparing content (not just "is it the newest row") is what makes the marker honest after
    // a save the ring dropped or a dedupe skip.
    //
    // A read failure here degrades to "no row is marked current" rather than failing the list: the
    // history is still the answer to the question that was asked.
    let live_hash = match read_versioned(store, ws, plan.table, id).await {
        Ok(v) => v.value.as_ref().map(|d| snapshot_hash(d, plan.hash_ignore)),
        Err(_) => None,
    };

    let versions = rows
        .iter()
        .map(|r| r.meta(live_hash.as_deref() == Some(r.hash.as_str())))
        .collect();

    let cap = resolve_cap(&read_config(store, ws).await.unwrap_or_default(), kind);
    Ok(VersionList { versions, cap })
}

/// The shared unknown-kind refusal — a typed error naming the kinds that DO exist, so a caller (or
/// a model) fixes the call instead of concluding the entity has no history.
pub fn unknown_kind(kind: &str) -> VersionsError {
    let known: Vec<&str> = super::plan::KIND_PLANS.iter().map(|p| p.kind).collect();
    VersionsError::BadInput(format!(
        "unknown kind `{kind}` — versioned kinds are: {}",
        known.join(", ")
    ))
}
