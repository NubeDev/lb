//! `GET /public/branding` — the **pre-auth** workspace brand read (workspace-branding scope, the
//! "public read seam" slice that was deferred when login branding first shipped).
//!
//! The sign-in screen runs with **no token**, so it cannot call `prefs.resolve` — every `prefs.*`
//! verb derives the workspace from the bearer. Until this route existed, a browser could only paint
//! the brand it had cached from a *previous* authenticated visit, so the first impression of a
//! branded deployment on a new device, a new browser, or a cleared profile was the product default.
//! That is not fixable in the shell: an unauthenticated read is required.
//!
//! This is therefore a **deliberate, opt-in, read-only break in the workspace wall** (§7), following
//! the document-store's public-serving precedent (README §6.12). Four properties keep it hairline —
//! change none of them without re-running `/security-review`:
//!
//! 1. **Whitelist by construction, not by filtering.** The handler destructures the loaded
//!    [`lb_prefs::Prefs`] into exactly two fields and builds the body from those. The record is
//!    never serialized whole, so a *future* prefs axis cannot leak here by simply existing — it
//!    would have to be added to this file by hand. (`prefs.get` serializes the record; this does not.)
//! 2. **Not a workspace-existence oracle.** Unknown workspace, unbranded workspace, malformed slug,
//!    missing `ws`, and a store failure all return the identical `200 {ui_branding:null,
//!    ui_theme:null}`. A caller learns nothing from the response that it did not already assert.
//! 3. **`ws` is required, never inferred.** There is no "the node's own workspace" fallback: a node
//!    would have to enumerate workspaces pre-auth to find one, which is the enumeration this route
//!    exists to avoid. The sign-in screen always knows which workspace it is signing into (the
//!    `#/t/<ws>` deep link, or the workspace field on the form), so it can always say so.
//! 4. **Rate-limited from day one**, per client, like `POST /public/invite/accept` — see
//!    `rate_limit.rs`.
//!
//! What it serves is public by construction: the brand and theme are exactly what the sign-in screen
//! paints for anyone who can reach the host.

use axum::extract::{Query, State};
use axum::http::{header, HeaderValue};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use crate::state::Gateway;

/// How long a brand response may be reused. Short on purpose (workspace-branding scope, "caching &
/// staleness"): login is a hot path and the brand is stable, but an admin re-brand must show up
/// promptly rather than being pinned in a browser cache for the rest of the session.
pub const BRANDING_MAX_AGE_SECS: u32 = 60;

/// `?ws=<workspace>` — the workspace whose brand to paint. `Option` rather than a required field so
/// a caller that omits it gets the same opaque empty brand as one that names an unknown workspace,
/// instead of an extractor `400` that distinguishes the two shapes.
#[derive(Debug, Deserialize)]
pub struct BrandingQuery {
    #[serde(default)]
    pub ws: Option<String>,
}

/// `GET /public/branding?ws=<ws>` — UNAUTHENTICATED. Returns the workspace-default `ui_branding` and
/// `ui_theme` blobs (opaque to Rust; the shell's `lib/branding` + theme layer parse them), and
/// nothing else. Both are `null` when the workspace has set no default, does not exist, or was not
/// named — one response shape for every miss (property 2 in the module docs).
pub async fn public_branding(
    State(gw): State<Gateway>,
    Query(q): Query<BrandingQuery>,
) -> Response {
    // `get_workspace_prefs` reads ONLY `workspace_prefs:[ws]` — the admin-owned workspace-default
    // link. It never touches the member link, so this route has no member record to leak even in
    // principle. An `Err` (including the store's own invalid-slug rejection, which is what guards
    // the namespace against injection) collapses into the same `None` as "unset".
    let prefs = match q.ws.as_deref().map(str::trim).filter(|ws| !ws.is_empty()) {
        Some(ws) => lb_prefs::get_workspace_prefs(&gw.node.store, ws)
            .await
            .ok()
            .flatten(),
        None => None,
    };

    // THE WALL. Two named fields off the record — never `serde_json::to_value(prefs)`. Adding a
    // field here is adding it to the public internet; that is the review this route exists to force.
    let (ui_branding, ui_theme) = match prefs {
        Some(p) => (p.ui_branding, p.ui_theme),
        None => (None, None),
    };

    (
        [(
            header::CACHE_CONTROL,
            HeaderValue::from_str(&format!("public, max-age={BRANDING_MAX_AGE_SECS}"))
                .unwrap_or(HeaderValue::from_static("public, max-age=60")),
        )],
        Json(json!({ "ui_branding": ui_branding, "ui_theme": ui_theme })),
    )
        .into_response()
}
