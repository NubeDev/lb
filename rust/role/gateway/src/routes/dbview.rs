//! DB-browser routes — the browser's `store.*` surface over the gateway (data-console scope). The
//! admin, **read-only** raw-store lens: list tables + counts, page raw rows, read the relation graph
//! for react-flow. Each route mirrors a `lb_host::store_*_view` 1:1 and re-runs the host's gate
//! server-side — workspace-first, then the **admin** capability (`mcp:store.tables/scan/graph:call`,
//! granted to the workspace-admin role only, NOT members). The workspace + principal come from the
//! **token**, never the request (the hard wall, §7).
//!
//! These verbs deliberately relax the per-record membership gate (gate 3) — a raw scan answers "every
//! record in the workspace" — so the admin-only cap is load-bearing. A denied caller is `403`-opaque
//! (no existence signal). There are **no write routes here by design** (read-only; edits go through
//! the domain verbs).

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_host::{DbViewError, Graph, Page, TableCount};
use serde::Deserialize;

use crate::session::authenticate;
use crate::state::Gateway;

/// `GET /store/tables` — list the workspace's tables + row counts (the picker). Admin cap.
pub async fn list_tables(
    State(gw): State<Gateway>,
    headers: HeaderMap,
) -> Result<Json<Vec<TableCount>>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let tables = lb_host::store_tables_view(&gw.node.store, &p, p.ws())
        .await
        .map_err(dbview_status)?;
    Ok(Json(tables))
}

/// `GET /store/tables/{table}/rows?limit=&cursor=` query — a bounded, id-cursor-paged page.
#[derive(Debug, Deserialize)]
pub struct ScanQuery {
    pub limit: Option<usize>,
    pub cursor: Option<String>,
}

/// `GET /store/tables/{table}/rows` — page raw rows of `table` (the grid). Admin cap. The `limit` is
/// hard-capped server-side; the response carries the next cursor (or `null` at the end).
pub async fn scan_table(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(table): Path<String>,
    Query(q): Query<ScanQuery>,
) -> Result<Json<Page>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let page = lb_host::store_scan_view(
        &gw.node.store,
        &p,
        p.ws(),
        &table,
        q.limit.unwrap_or(50),
        q.cursor.as_deref(),
    )
    .await
    .map_err(dbview_status)?;
    Ok(Json(page))
}

/// `GET /store/graph?table=&id=&depth=` query — seed the graph from a table and/or a record id.
#[derive(Debug, Deserialize)]
pub struct GraphQuery {
    pub table: Option<String>,
    pub id: Option<String>,
    pub depth: Option<u32>,
}

/// `GET /store/graph` — a depth/fan-out-bounded slice of nodes + relation edges for react-flow.
/// Admin cap. At least one of `table`/`id` should be given (else an empty graph).
pub async fn read_graph(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Query(q): Query<GraphQuery>,
) -> Result<Json<Graph>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let g = lb_host::store_graph_view(
        &gw.node.store,
        &p,
        p.ws(),
        q.table.as_deref(),
        q.id.as_deref(),
        q.depth.unwrap_or(1),
    )
    .await
    .map_err(dbview_status)?;
    Ok(Json(g))
}

/// Map the DB-browser gate's outcome onto an HTTP status. `Denied` is `403` (opaque — no existence
/// signal); a store fault is `403`-opaque like every other gateway route.
fn dbview_status(e: DbViewError) -> (StatusCode, String) {
    (StatusCode::FORBIDDEN, e.to_string())
}
