//! The public inbound webhook route — `POST /hooks/{ws}/{id}` (webhooks scope). This is the
//! **only** unauthenticated-by-session route in the gateway: a third-party service calls it from
//! outside, presenting the webhook's own credential (a bearer apikey or an HMAC signature), not a
//! workspace session token. The route:
//!
//! 1. Resolves `{ws}` + `{id}` from the URL (O(1) — no scan, mirroring the apikey `lbk_{ws}.…`
//!    grammar).
//! 2. Captures the **raw body BEFORE any JSON parse** (load-bearing: HMAC verify runs over the
//!    exact received bytes; a re-serialized body breaks every real signature — the most common
//!    webhook-integration bug).
//! 3. Calls `webhook_resolve` — load record → per-mode verify → Principal. Every failure
//!    (unknown id / disabled / wrong-secret / cross-ws) collapses to the same opaque `404`/`401`
//!    so the public route is **not a webhook-id oracle** (no existence leak).
//! 4. Calls `webhook_accept` — build a `Sample`, write through `ingest.write`, drain+publish
//!    motion, bump `last_hit_at`.
//! 5. Replies `202 Accepted { id, series, seq }` (the sample's coordinate — a sender can poll
//!    `series.read` to confirm commit, but the buffer's acceptance IS the durability promise).
//!
//! The route holds **no business logic** — it is an auth-and-normalize edge. Provider shaping (a
//! Slack/GitHub/Stripe payload parser) is a downstream extension or flow node the user opts into,
//! NEVER a branch here (rule 10).

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_host::{webhook_accept, webhook_resolve};
use serde::Serialize;

use crate::state::Gateway;

/// The inbound endpoint. `POST /hooks/{ws}/{id}` with the raw body and the mode-appropriate
/// credential header. Returns `202 Accepted` on success; `404` for unknown/disabled/cross-ws
/// (opaque); `401` for a wrong/absent secret (opaque); `410` for a revoked hook.
pub async fn post_webhook(
    State(gw): State<Gateway>,
    Path((ws, id)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<(StatusCode, Json<Accepted>), (StatusCode, String)> {
    let bearer_value = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    // The signature header is admin-picked (recorded on the webhook row); we don't know its name
    // ahead of time, so pass a closure that looks up a header by NAME — `webhook_resolve` reads
    // the record's `hmac_header` and asks the closure for that exact header. Header-name lookup is
    // case-insensitive in `HeaderMap` (the route stays transport-agnostic; the host sees a fn).
    let header_lookup = |name: &str| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
    };

    let now_secs = gw.now();
    let now_ms = now_secs.saturating_mul(1000);

    let (record, principal) = match webhook_resolve(
        &gw.node.store,
        &gw.node.apikeys,
        &gw.pepper,
        &ws,
        &id,
        &body,
        bearer_value.as_deref(),
        header_lookup,
        now_secs,
    )
    .await
    {
        Ok(ok) => ok,
        Err(e) => return Err(map_inbound_err(e)),
    };

    let method = "POST";
    let sample = webhook_accept(
        &gw.node.store,
        &gw.node.bus,
        &gw.node.apikeys,
        &gw.pepper,
        &principal,
        &ws,
        &record,
        &body,
        method,
        now_secs,
        now_ms,
    )
    .await
    .map_err(map_inbound_err)?;

    Ok((
        StatusCode::ACCEPTED,
        Json(Accepted {
            id: record.id.clone(),
            series: sample.series,
            seq: sample.seq,
        }),
    ))
}

/// The `202 Accepted` body: the sample's coordinate, so a sender can poll `series.read`/`latest`
/// to confirm the hit committed. Minimal — never echoes the payload (which may be large / sensitive).
#[derive(Debug, Serialize)]
pub struct Accepted {
    pub id: String,
    pub series: String,
    pub seq: u64,
}

/// Map the inbound service error to an HTTP status. Every auth-shaped failure (unknown id,
/// disabled, wrong-secret, cross-ws URL) collapses to the same opaque `404` so the public route
/// is not a webhook-id oracle (no existence leak). A revoked hook is `410 Gone`. A store error
/// stays opaque (`502` — the gateway cannot serve the request, but reveals nothing).
fn map_inbound_err(e: lb_host::WebhookError) -> (StatusCode, String) {
    use lb_host::WebhookError;
    match e {
        // Opaque existence-leak guards: every "is this id live / did the secret verify" failure
        // looks identical to the caller.
        WebhookError::NotFound | WebhookError::Invalid => {
            (StatusCode::NOT_FOUND, "not found".into())
        }
        WebhookError::Revoked => (StatusCode::GONE, "gone".into()),
        WebhookError::Denied => (StatusCode::FORBIDDEN, "denied".into()),
        WebhookError::BadInput(m) => (StatusCode::BAD_REQUEST, m),
        WebhookError::Widen(c) => (
            StatusCode::BAD_REQUEST,
            format!("cannot grant a cap the creator lacks: {c}"),
        ),
        WebhookError::Store(_) => (StatusCode::BAD_GATEWAY, "unavailable".into()),
    }
}
