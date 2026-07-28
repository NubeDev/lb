//! `series.retention.*` — the capability-gated administration of series retention (series-retention
//! scope, issue #58). Three verbs, each its own MCP surface + cap:
//!   - `series.retention.set` — upsert the policy for a series-name prefix;
//!   - `series.retention.list` — the workspace's policies;
//!   - `series.retention.gc` — run one rollup-then-evict pass now (`now_ms` is the caller's logical
//!     clock; the HTTP/MCP layer stamps wall-clock when the caller omits it).
//!
//! Namespace-scoped like every series verb (the hard wall); a denial is opaque.

use lb_auth::Principal;
use lb_ingest::{delete_policy, list_policies, run_gc, set_policy, Filter, GcPass, Policy, Tier};
use lb_store::Store;
use serde_json::Value;

use super::authorize::authorize_ingest;
use super::error::IngestError;

/// Upsert `policy` in `ws`, stamping provenance. Gated by `mcp:series.retention.set:call`.
///
/// **This REPLACES the whole row** — any field the caller omits reverts to its default. That is the
/// correct semantic for a "set" (it is how you remove a filter or a tier), but it is also a footgun
/// for a hand-written body: a live `modbus.` policy lost its tier `method` exactly this way, and
/// nothing recorded that it had happened. [`series_retention_patch`] is the merge-preserving verb
/// for read-modify-write callers; the returned policy is what was actually stored, so a caller can
/// SEE what a replace cost them instead of discovering it days later in a panel.
pub async fn series_retention_set(
    store: &Store,
    principal: &Principal,
    ws: &str,
    mut policy: Policy,
    now_ms: u64,
) -> Result<Policy, IngestError> {
    authorize_ingest(principal, ws, "series.retention.set")?;
    // An empty prefix matches EVERY series (`resolve_policy` is `starts_with`), so it is a silent
    // catch-all that outranks nothing and is outranked by everything — almost never what anyone
    // means. It became reachable when `Policy` gained `Default` (a `..Default::default()` spread
    // that forgets `prefix` now compiles), so the wall goes here rather than trusting call sites.
    if policy.prefix.trim().is_empty() {
        return Err(IngestError::BadInput(
            "prefix must not be empty — an empty prefix silently governs every series".into(),
        ));
    }
    stamp(&mut policy, principal, now_ms);
    set_policy(store, ws, &policy).await?;
    Ok(policy)
}

/// Stamp the authenticated writer onto the row. NEVER caller-supplied — same posture as the producer
/// root on `ingest.write`: a provenance field a caller can forge answers nothing.
fn stamp(policy: &mut Policy, principal: &Principal, now_ms: u64) {
    policy.updated_by = Some(principal.sub().to_string());
    policy.updated_ms = Some(now_ms);
}

/// Merge `changes` into the policy at `prefix` and store the result. Gated by
/// `mcp:series.retention.set:call` — patching is the SAME administrative privilege as setting, so no
/// new capability is minted (the identical argument `series_retention_delete` already makes).
///
/// # Semantics, stated because a merge is only useful if it is predictable
///
/// - A key absent from `changes` leaves the stored value **untouched**. That is the whole point:
///   read-modify-write in one call, with no window in which a concurrent writer's field is lost, and
///   no way to drop a field by forgetting it.
/// - `tiers`, when supplied, **replaces the list** (so a tier can be removed) — but each supplied
///   tier is merged FIELD-WISE with the stored tier at the same `width_ms`. So re-sending a tier
///   without its `method` keeps the method it already had. This is the precise fix for the reported
///   bug: the list is authored wholesale, the fields within it are not.
/// - `filter` supplied as `null` CLEARS it; absent leaves it. The distinction needs the raw JSON,
///   which is why this takes a `Value` rather than a typed struct — `Option<Filter>` cannot tell
///   "absent" from "explicitly null", and collapsing them would make one of the two impossible.
///
/// Patching a prefix that has no policy is a `BadInput`, not a silent create: a caller who patches
/// a prefix they believe exists has made a mistake, and inventing a row from partial fields is how
/// a half-configured policy gets written in the first place.
pub async fn series_retention_patch(
    store: &Store,
    principal: &Principal,
    ws: &str,
    prefix: &str,
    changes: &Value,
    now_ms: u64,
) -> Result<Policy, IngestError> {
    authorize_ingest(principal, ws, "series.retention.set")?;

    let existing = list_policies(store, ws)
        .await?
        .into_iter()
        .find(|p| p.prefix == prefix)
        .ok_or_else(|| {
            IngestError::BadInput(format!(
                "no retention policy at prefix `{prefix}` — use series.retention.set to create one"
            ))
        })?;

    let merged = merge(existing, changes)?;
    series_retention_set(store, principal, ws, merged, now_ms).await
}

/// Apply `changes` to `base`. Pure — every store and capability concern stays in the caller, so the
/// merge rules above are testable on their own.
fn merge(mut base: Policy, changes: &Value) -> Result<Policy, IngestError> {
    let obj = changes
        .as_object()
        .ok_or_else(|| IngestError::BadInput("changes must be a JSON object".into()))?;

    if let Some(v) = obj.get("raw_for_ms") {
        base.raw_for_ms = num(v, "raw_for_ms")?;
    }
    if let Some(v) = obj.get("max_samples") {
        base.max_samples = num(v, "max_samples")?;
    }
    if let Some(v) = obj.get("filter") {
        // Present-and-null is a deliberate CLEAR; absent never reaches here.
        base.filter = if v.is_null() {
            None
        } else {
            Some(
                serde_json::from_value::<Filter>(v.clone())
                    .map_err(|e| IngestError::BadInput(format!("filter: {e}")))?,
            )
        };
    }
    if let Some(v) = obj.get("tiers") {
        let supplied = v
            .as_array()
            .ok_or_else(|| IngestError::BadInput("tiers must be an array".into()))?;
        let mut out = Vec::with_capacity(supplied.len());
        for t in supplied {
            out.push(merge_tier(&base.tiers, t)?);
        }
        base.tiers = out;
    }
    Ok(base)
}

/// Merge one supplied tier with the stored tier of the same width, so a field the caller did not
/// mention survives. `width_ms` is required — it is the identity of a tier, and a merge with no
/// identity is just a replace wearing a disguise.
fn merge_tier(stored: &[Tier], supplied: &Value) -> Result<Tier, IngestError> {
    let obj = supplied
        .as_object()
        .ok_or_else(|| IngestError::BadInput("each tier must be a JSON object".into()))?;
    let width_ms = num(
        obj.get("width_ms")
            .ok_or_else(|| IngestError::BadInput("each tier needs width_ms".into()))?,
        "width_ms",
    )?;

    let mut tier = stored
        .iter()
        .find(|t| t.width_ms == width_ms)
        .cloned()
        .unwrap_or_default();
    tier.width_ms = width_ms;

    if let Some(v) = obj.get("keep_for_ms") {
        tier.keep_for_ms = num(v, "keep_for_ms")?;
    }
    if let Some(v) = obj.get("method") {
        tier.method = if v.is_null() {
            None
        } else {
            Some(
                serde_json::from_value(v.clone())
                    .map_err(|e| IngestError::BadInput(format!("method: {e}")))?,
            )
        };
    }
    Ok(tier)
}

fn num(v: &Value, field: &str) -> Result<u64, IngestError> {
    v.as_u64()
        .ok_or_else(|| IngestError::BadInput(format!("{field} must be a non-negative integer")))
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
