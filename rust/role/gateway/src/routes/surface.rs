//! `GET /surface/{surface}` — the **page-reach preflight** (nav-reach scope): may the caller OPEN this
//! core page? Returns `200` if their resolved nav grants it (`reach:<surface>:view`, or the fallback
//! wildcard), `403` otherwise. A gated page's client loader calls this ONCE on mount; a `403` redirects
//! it to the caller's default page.
//!
//! **Why a dedicated route** (not gating the existing list routes). The pages a curated nav must block
//! (Ingest, Rules, Flows, Datasources) load via list routes (`GET /series`, `/rules`, `/flows`,
//! `/datasources`) that are ALSO called by dashboard tiles, the Data Studio source picker, the nav
//! editor, and the channel pin picker — so the server cannot tell "open the Ingest page" from "a tile
//! lists series"; they are the same request. Gating those would break the pages the caller IS allowed
//! to see. This route is the page's OWN entry, distinct from every shared data route: the data routes
//! stay open (tiles/pickers keep working), and *opening the page* is a hard server boundary. Curling a
//! shared list still returns the same data a tile sees — but the caller cannot LOAD a page not in their
//! nav, which is the reach restriction that was asked for.
//!
//! **Rule 10:** `{surface}` is opaque data — the gate is `reach:<surface>:view ∈ caps?`, generic over
//! the key, never a `match surface { "rules" => … }`. The boundary is the same `lb_caps::check`
//! primitive every other cap rides (via `require_reach`).

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde_json::{json, Value};

use crate::session::require_reach;
use crate::state::Gateway;

/// `GET /surface/{surface}` — `200 {reachable:true, surface}` iff the caller holds
/// `reach:<surface>:view` (or the fallback wildcard `reach:*:view`); an opaque `403` otherwise. The
/// body carries no page data — this is a pure gate the client awaits before rendering the page.
pub async fn surface_reach(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(surface): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let _p = require_reach(&gw, &headers, &surface).await?;
    Ok(Json(json!({ "reachable": true, "surface": surface })))
}
