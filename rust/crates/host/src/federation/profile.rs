//! `federation.profile {source, tables?}` → compute a discovery profile for a registered source and
//! **upsert it** as `datasource_profile:{ws}:{source}` (datasource-profile scope).
//!
//! This is the WRITE half. It reuses `federation.sample`'s exact gated pipeline — resolve the source
//! IN THE CALLER'S WORKSPACE, enforce `net:*`, mediate the DSN, ONE supervised-sidecar call — and
//! authorizes under the SAME read cap (`mcp:federation.query:call`): a profile is strictly less than
//! what the read cap can already `SELECT`, so no new capability is introduced for it.
//!
//! It is bounded-synchronous by design, not a fast verb. The hot read path is
//! [`profile_get`](super::profile_get), a pure store read; this one is reached from
//! `federation.profile_refresh`'s job, from a register-time enqueue, or from an explicit call.

use lb_auth::Principal;
use lb_supervisor::Launcher;
use serde_json::{json, Value};

use super::authorize::authorize;
use super::error::FederationError;
use super::net::{enforce_endpoint, FEDERATION_EXT};
use super::profile_record::{put, DatasourceProfile};
use super::record::resolve;
use super::secret::mediate_dsn;
use crate::boot::Node;

/// The bounds a pass runs under. Mirrors `BootConfig`'s `ProfileConfig`; the sidecar clamps these
/// again to its compile-time ceilings, so a config value can only make a pass CHEAPER (defense in
/// depth, the `federation.sample` limit-clamp precedent).
#[derive(Debug, Clone, Copy)]
pub struct ProfileBounds {
    pub max_tables: u64,
    pub max_values: u64,
}

impl Default for ProfileBounds {
    fn default() -> Self {
        Self {
            max_tables: 25,
            max_values: 60,
        }
    }
}

/// Profile `source` in `ws` as `caller`, persist the result, and return the stored record as JSON.
/// `tables` filters to the named tables when present. The DSN is mediated host-side, never returned.
// Argument count is the explicit dependency list; bundling it into a struct would be a refactor.
#[allow(clippy::too_many_arguments)]
pub async fn federation_profile<L: Launcher>(
    node: &Node,
    launcher: &L,
    caller: &Principal,
    ws: &str,
    source: &str,
    tables: Option<&[String]>,
    bounds: ProfileBounds,
    ts: u64,
) -> Result<Value, FederationError> {
    // Profiling is the same read privilege as a live query — authorize under the read cap so no new
    // capability grant is needed (same decision as `federation.schema`/`federation.sample`).
    authorize(caller, ws, "federation.query")?;

    // Resolve the alias to a registered source IN THIS workspace — un-spoofable (the wall).
    let ds = resolve(&node.store, ws, source)
        .await?
        .ok_or(FederationError::NotFound)?;

    // `net:*` — refuse, opaque, if the source's endpoint is not in the admin-approved grant.
    enforce_endpoint(&node.store, ws, &ds.endpoint).await?;

    // Mediate the DSN under the FEDERATION extension's own grant (never the caller's).
    let dsn = mediate_dsn(node, ws, &ds.secret_ref).await?;

    let mut input = json!({
        "kind": ds.kind,
        "dsn": dsn,
        "source": source,
        "max_tables": bounds.max_tables,
        "max_values": bounds.max_values,
    });
    if let Some(tables) = tables {
        input["tables"] = json!(tables);
    }
    let input = input.to_string();

    let out = crate::native::call_sidecar_mediated(
        node,
        launcher,
        caller,
        ws,
        FEDERATION_EXT,
        "federation.profile",
        &input,
        ts,
    )
    .await
    .map_err(|e| FederationError::Sidecar(e.to_string()))?;

    let pass: Value =
        serde_json::from_str(&out).map_err(|e| FederationError::Sidecar(e.to_string()))?;

    // Upsert. `from_pass` clears `profiling_since`, so landing the record also releases the
    // reactor's in-flight guard — one write, no separate unlock step to leak on an error path.
    let rec = DatasourceProfile::from_pass(source, &pass, ts);
    put(&node.store, ws, &rec).await?;
    Ok(serde_json::to_value(&rec).unwrap_or(Value::Null))
}

/// The palette/agent descriptor for `federation.profile` — a real arg schema so a model advertised
/// the tool can FORM a valid call. `x-lb entity: datasource` drives the same `@`-picker as siblings.
pub fn profile_descriptor() -> lb_mcp::ToolDescriptor {
    lb_mcp::ToolDescriptor {
        emits_external: false,
        name: "federation.profile".to_string(),
        title: "Profile a datasource: per-column cardinality, values, ranges (computes + stores)"
            .to_string(),
        group: "federation".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "source": { "type": "string", "x-lb": { "entity": "datasource" } },
                "tables": { "type": "array" }
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
