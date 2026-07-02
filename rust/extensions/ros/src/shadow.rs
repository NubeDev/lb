//! The **config shadow** — the minimal driver records the sidecar persists, over the host's
//! workspace-scoped `assets.*` document store (reached through the callback client). A native sidecar
//! holds no store handle of its own (rule 4: stateless; the store is the truth), and there is no
//! generic host "put a record" verb — `assets.put_doc`/`get_doc`/`list_docs`/`delete_doc` IS the
//! host-mediated, workspace-isolated, capability-gated key→document store, so the shadow rides it.
//!
//! **What is shadowed (minimal, per the resolved scope decision):** only the `ros` **connection**
//! records (`{uuid, name, base_url, enable, poll_rate, parent:null}`) and, later, per-node enable /
//! poll overrides. The box stays the authority for the network/device/point tree — those are proxied
//! live on `get`/`list` (RosApi), never copied here. The `External` token is NOT in the shadow — it is
//! mediated by `lb-secrets` (`secret:ros/{uuid}/token`) and never returned by `get`/`list`.
//!
//! **Id scheme (workspace isolation is the host's, by the callback token — see `host.rs`):** a doc id
//! `ros:{ros_uuid}`. `list` fetches all doc ids and filters by the `ros:` prefix, then reads each — the
//! config tree is low-cardinality and low-rate (motion goes through `series`, not here), so an
//! N+1 read on `list` is fine and keeps us on the generic doc store with no schema.

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::host::{HostCtx, HostError};

/// A `ros` connection config shadow — the minimum needed to schedule polling and render the fleet
/// without a round-trip to the box. `parent` is `None` for a connection (it is the tree root); it is
/// carried uniformly so the same record shape can shadow lower nodes' overrides later.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RosShadow {
    pub uuid: String,
    pub name: String,
    pub base_url: String,
    #[serde(default = "default_true")]
    pub enable: bool,
    /// Platform-side poll interval (seconds), seeded from the box but operator-overridable — the
    /// resolved scope decision on poll-rate source.
    #[serde(default)]
    pub poll_rate: Option<u64>,
    #[serde(default)]
    pub parent: Option<String>,
}

fn default_true() -> bool {
    true
}

/// The doc id for a connection shadow. `ros/{uuid}` — a `/` separator (NOT `:`) because the doc id
/// becomes the capability resource `store:doc/{id}:{action}`, and the cap grammar splits on `:` and
/// wildcards per `/` segment. So `ros/{uuid}` → cap `store:doc/ros/*:write` (the `*` matches the uuid
/// segment); a `:` in the id would collide with the cap's action delimiter. The `ros/` prefix is what
/// `list` filters on.
pub fn ros_doc_id(uuid: &str) -> String {
    format!("ros/{uuid}")
}

const ROS_PREFIX: &str = "ros/";

/// Upsert a connection shadow (`assets.put_doc`). The record's JSON is the doc `content`; the title is
/// the human name (what `list_docs` returns cheaply). Idempotent on the id (a re-put overwrites).
pub async fn put_ros(host: &HostCtx, shadow: &RosShadow, ts: u64) -> Result<(), HostError> {
    let content =
        serde_json::to_string(shadow).map_err(|e| HostError::BadResponse(e.to_string()))?;
    host.client()
        .call_tool(
            "assets.put_doc",
            json!({
                "id": ros_doc_id(&shadow.uuid),
                "title": shadow.name,
                "content": content,
                "content_type": "application/json",
                "tags": ["ros-connection"],
                "ts": ts,
            }),
        )
        .await?;
    Ok(())
}

/// Read one connection shadow (`assets.get_doc` → parse `content`). `Ok(None)` when the doc is absent
/// (a `Denied`/other host error is surfaced, not swallowed — a missing doc is distinct from a refusal).
pub async fn get_ros(host: &HostCtx, uuid: &str) -> Result<Option<RosShadow>, HostError> {
    let out = match host
        .client()
        .call_tool("assets.get_doc", json!({ "id": ros_doc_id(uuid) }))
        .await
        .map_err(HostError::from)
    {
        Ok(v) => v,
        // The gateway maps EVERY ToolError (incl. NotFound) to an opaque `403` (the no-existence-oracle
        // MCP contract), so a missing doc reaches us as `Denied`, indistinguishable from a real refusal.
        // Within our OWN workspace the sidecar's grant covers every `ros/**` doc, so a `Denied` here can
        // only mean "absent" — treat it (and any "not found" text) as `None`, a clean 404 for `ros.get`.
        // A cap misconfiguration would also land here, but that surfaces at `create`/`list` (which the
        // deny test asserts), not as a silent read failure.
        Err(HostError::Denied) => return Ok(None),
        Err(HostError::Callback(m)) if m.contains("not found") => return Ok(None),
        Err(e) => return Err(e),
    };
    let content = out
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| HostError::BadResponse("get_doc: no content".into()))?;
    let shadow: RosShadow =
        serde_json::from_str(content).map_err(|e| HostError::BadResponse(e.to_string()))?;
    Ok(Some(shadow))
}

/// List all connection shadows in the workspace: `assets.list_docs` returns every doc id, we filter to
/// the `ros:` prefix and read each. Low-cardinality config, so the N+1 read is acceptable (see header).
pub async fn list_ros(host: &HostCtx) -> Result<Vec<RosShadow>, HostError> {
    let out = host
        .client()
        .call_tool("assets.list_docs", json!({}))
        .await?;
    let docs = out
        .get("docs")
        .and_then(|v| v.as_array())
        .ok_or_else(|| HostError::BadResponse("list_docs: no docs array".into()))?;

    let mut shadows = Vec::new();
    for d in docs {
        let id = d.get("id").and_then(|v| v.as_str()).unwrap_or_default();
        if let Some(uuid) = id.strip_prefix(ROS_PREFIX) {
            if let Some(shadow) = get_ros(host, uuid).await? {
                shadows.push(shadow);
            }
        }
    }
    Ok(shadows)
}

/// Delete a connection shadow (`assets.delete_doc`). The secret token is deleted separately by the
/// handler (`secret.delete`); this removes only the config record.
pub async fn delete_ros(host: &HostCtx, uuid: &str) -> Result<(), HostError> {
    host.client()
        .call_tool("assets.delete_doc", json!({ "id": ros_doc_id(uuid) }))
        .await?;
    Ok(())
}
