//! System-map routes — the browser's `system.*` surface over the gateway (system-map scope). The
//! admin, **read-only** workspace topology + status console: a per-subsystem status grid and a
//! react-flow wiring graph, both projected from one live snapshot. Each route mirrors a
//! `lb_host::system_*` verb 1:1 and re-runs the host's gate server-side — workspace-first, then the
//! **admin** capability (`mcp:system.overview/topology:call`, granted to the workspace-admin role
//! only, NOT members). The workspace + principal come from the **token**, never the request (§7).
//!
//! A snapshot reads across every subsystem of the workspace, so the admin-only cap is load-bearing.
//! A denied caller is `403`-opaque (no existence signal). There are **no write routes here by
//! design** (read-only; control verbs live in their own scopes).

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_host::{AcpInfo, SubsystemDetail, SystemError, SystemOverview, SystemTools, SystemTopology};

use crate::session::authenticate;
use crate::state::Gateway;

/// `GET /system/overview` — the per-subsystem status grid for the caller's workspace. Admin cap.
pub async fn system_overview(
    State(gw): State<Gateway>,
    headers: HeaderMap,
) -> Result<Json<SystemOverview>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers).map_err(|e| e.into_response())?;
    let ov = lb_host::system_overview(gw.node.as_ref(), &p, p.ws())
        .await
        .map_err(system_status)?;
    Ok(Json(ov))
}

/// `GET /system/topology` — nodes + wiring edges for the react-flow graph. Admin cap.
pub async fn system_topology(
    State(gw): State<Gateway>,
    headers: HeaderMap,
) -> Result<Json<SystemTopology>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers).map_err(|e| e.into_response())?;
    let topo = lb_host::system_topology(gw.node.as_ref(), &p, p.ws())
        .await
        .map_err(system_status)?;
    Ok(Json(topo))
}

/// `GET /system/subsystem/{id}` — the full detail of one subsystem (the same card the grid shows,
/// plus a subsystem-specific `extra` blob; for `bus`, its live peer/router zid lists). The detail
/// view a no-page card drills into. Admin cap; an unknown id is `403`-opaque, like a denial.
pub async fn system_subsystem(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<SubsystemDetail>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers).map_err(|e| e.into_response())?;
    let detail = lb_host::system_subsystem(gw.node.as_ref(), &p, p.ws(), &id)
        .await
        .map_err(system_status)?;
    Ok(Json(detail))
}

/// `GET /system/tools` — the full catalog of MCP tools reachable for the caller's workspace
/// (host-native + extension-contributed), with descriptions. The read behind the MCP service page's
/// tool table. Admin cap (`mcp:system.tools:call`).
pub async fn system_tools(
    State(gw): State<Gateway>,
    headers: HeaderMap,
) -> Result<Json<SystemTools>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers).map_err(|e| e.into_response())?;
    let tools = lb_host::system_tools(gw.node.as_ref(), &p, p.ws())
        .await
        .map_err(system_status)?;
    Ok(Json(tools))
}

/// `GET /system/acp` — the ACP adapter's static protocol/capability facts (the read behind the ACP
/// service page). Admin cap (`mcp:system.acp:call`). Node-level facts, but gated workspace-first like
/// its siblings so the page is admin-only.
pub async fn system_acp(
    State(gw): State<Gateway>,
    headers: HeaderMap,
) -> Result<Json<AcpInfo>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers).map_err(|e| e.into_response())?;
    let info = lb_host::system_acp(&p, p.ws())
        .await
        .map_err(system_status)?;
    Ok(Json(info))
}

/// Map the system gate's outcome onto an HTTP status. `Denied` is `403` (opaque — no existence
/// signal); a store fault is `403`-opaque like every other gateway route.
fn system_status(e: SystemError) -> (StatusCode, String) {
    (StatusCode::FORBIDDEN, e.to_string())
}
