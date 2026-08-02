//! `federation.profile_get {source, compute_if_missing?}` → the stored discovery profile
//! (datasource-profile scope). **This is the hot path**, and it is a PURE STORE READ: one record
//! lookup, no external DB touch, no sidecar, no wait.
//!
//! That purity is the whole point. A UI opening a chart composer, or an agent being handed a
//! source's shape as context, must get an answer in a store read or an honest `NotFound` — it must
//! never be silently converted into a 20 s profiling pass because a record happened to be absent.
//! Callers that genuinely want the compute opt in EXPLICITLY with `compute_if_missing: true`, and by
//! passing it they are stating they can afford to block.
//!
//! Authorized under the existing read cap (`mcp:federation.query:call`) — the profile is strictly
//! less than what that cap can already `SELECT`.

use lb_auth::Principal;
use lb_supervisor::Launcher;
use serde_json::{json, Value};

use super::authorize::authorize;
use super::error::FederationError;
use super::profile::{federation_profile, ProfileBounds};
use super::profile_record::resolve as resolve_profile;
use super::record::resolve;
use crate::boot::Node;

/// Read `source`'s stored profile in `ws` as `caller`.
///
/// `NotFound` when the source is not registered here (the workspace wall: a ws-B caller naming a
/// ws-A source finds nothing) AND when the source is registered but never profiled — the caller
/// cannot tell those apart, which is the correct opaque posture.
///
/// With `compute_if_missing`, a miss runs one bounded pass inline and returns the fresh record. The
/// authorization for that is the same cap already checked here, so the flag grants nothing extra —
/// it only opts into the cost.
#[allow(clippy::too_many_arguments)]
pub async fn federation_profile_get<L: Launcher>(
    node: &Node,
    launcher: &L,
    caller: &Principal,
    ws: &str,
    source: &str,
    compute_if_missing: bool,
    bounds: ProfileBounds,
    ts: u64,
) -> Result<Value, FederationError> {
    authorize(caller, ws, "federation.query")?;

    // Resolve the ALIAS first, so an unregistered/cross-workspace name is `NotFound` even if a
    // stale profile record somehow survived the source's removal.
    if resolve(&node.store, ws, source).await?.is_none() {
        return Err(FederationError::NotFound);
    }

    if let Some(rec) = resolve_profile(&node.store, ws, source).await? {
        return Ok(serde_json::to_value(&rec).unwrap_or(Value::Null));
    }
    if compute_if_missing {
        return federation_profile(node, launcher, caller, ws, source, None, bounds, ts).await;
    }
    Err(FederationError::NotFound)
}

/// The palette/agent descriptor for `federation.profile_get`.
pub fn profile_get_descriptor() -> lb_mcp::ToolDescriptor {
    lb_mcp::ToolDescriptor {
        emits_external: false,
        name: "federation.profile_get".to_string(),
        title: "Read a datasource's stored discovery profile (one store read)".to_string(),
        group: "federation".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "source": { "type": "string", "x-lb": { "entity": "datasource" } },
                "compute_if_missing": { "type": "boolean" }
            },
            "required": ["source"]
        })),
        result: Some(json!({
            "v": 2,
            "view": "jsonview",
            "source": { "tool": "federation.profile_get", "args": {} },
            "options": { "collapsed": true },
            "tools": ["federation.profile_get", "federation.profile_refresh"]
        })),
    }
}
