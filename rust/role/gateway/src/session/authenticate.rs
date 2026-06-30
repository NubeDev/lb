//! Authenticate a request: read the bearer credential and verify it (api-keys scope: a bearer is
//! now EITHER a signed JWT OR an API key `lbk_{ws}.{id}.{secret}`).
//!
//! This is the gateway's identity chokepoint. A missing/garbled header is `401` (unauthenticated);
//! a forged, expired, revoked, or wrong-secret credential is `401`. On success the returned
//! [`Principal`] carries the workspace + caps — from the JWT for a human session, or resolved from
//! the durable grant store for an API key — so a route can never read the workspace from the request
//! body and the workspace wall (§7) holds at the front door for BOTH credential kinds.
//!
//! The two paths share one chokepoint and one `Principal` shape: an API key is just a non-human
//! `Subject` in the authz model, verified per request and authorized through `caps::check`. A JWT is
//! verified synchronously with the node key; an API key is verified async (a store read + HMAC) and
//! is cached briefly (busted on revoke). Every auth failure collapses to the same opaque `401` so a
//! caller cannot distinguish "no credential" / "bad JWT" / "unknown/revoked/expired/wrong-secret
//! key" (no oracle).

use axum::http::header::AUTHORIZATION;
use axum::http::{HeaderMap, StatusCode};
use lb_apikey::PREFIX;
use lb_auth::{verify, Principal};
use lb_host::{token_revoked, Subject};

use crate::state::Gateway;

/// Why authentication failed — all variants collapse to `401` so a caller can't distinguish "no
/// credential" from "bad credential" (no oracle), but kept distinct in the message for dev ergonomics.
#[derive(Debug)]
pub enum AuthRejection {
    /// No `Authorization: Bearer` header, or it was malformed.
    Missing,
    /// The credential failed verification (bad JWT signature/expiry, or an unknown/revoked/expired/
    /// wrong-secret API key — indistinguishable on purpose).
    Invalid,
}

impl AuthRejection {
    /// Map to the HTTP response a route returns. Always `401` — authenticity is decided before
    /// authority (a `403` would leak that the credential was valid but ungranted).
    pub fn into_response(self) -> (StatusCode, String) {
        let msg = match self {
            AuthRejection::Missing => "missing bearer credential",
            AuthRejection::Invalid => "invalid or expired credential",
        };
        (StatusCode::UNAUTHORIZED, msg.to_string())
    }
}

/// Verify the request's bearer credential against the node key (JWT) or the API-key store, returning
/// the verified principal. The single entry every guarded route uses to learn "who is calling". A
/// bearer beginning with the API-key prefix is verified as an API key; any other bearer is a JWT.
pub async fn authenticate(gw: &Gateway, headers: &HeaderMap) -> Result<Principal, AuthRejection> {
    let raw = headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or(AuthRejection::Missing)?;
    let token = raw
        .strip_prefix("Bearer ")
        .or_else(|| raw.strip_prefix("bearer "))
        .ok_or(AuthRejection::Missing)?
        .trim();
    verify_token(gw, token).await
}

/// Verify a bare credential string (no header framing) — the SSE path. `EventSource` cannot set an
/// `Authorization` header, so the stream route carries the credential as a `?token=` query param and
/// hands the raw value here. The verification is identical (JWT or API key) to [`authenticate`].
pub async fn verify_token(gw: &Gateway, token: &str) -> Result<Principal, AuthRejection> {
    let token = token.trim();
    if token.is_empty() {
        return Err(AuthRejection::Missing);
    }
    let principal = if token.starts_with(PREFIX) {
        // API-key path: the bearer carries its own workspace + key id, verified against the store.
        authenticate_apikey(gw, token).await?
    } else {
        // JWT path: verify signature + expiry with the node key at the gateway's clock.
        verify(&gw.key, token, gw.now).map_err(|_| AuthRejection::Invalid)?
    };
    // Live-token revoke (access-console scope): a per-(ws, subject) tombstone marker — written by
    // `authz.revoke-tokens` — refuses the subject's *current* (cached) token on the next request,
    // closing the freshness-asymmetry gap `revoke_subject` leaves open (that one bites on the next
    // re-mint). The marker syncs idempotently (§6.8); worst-case multi-node window = TTL. We check
    // it HERE (the one verify chokepoint) so a marked subject's bearer is treated as expired, as
    // opaque `Invalid` (indistinguishable from a genuinely expired/revoked credential — no oracle).
    // A store read error is deny-by-default-false here only to avoid a store outage locking out
    // everyone; a genuinely revoked subject whose marker read fails is still bounded by TTL/expiry.
    if is_live_revoked(gw, &principal).await {
        return Err(AuthRejection::Invalid);
    }
    Ok(principal)
}

/// Did `authz.revoke-tokens` mark this principal's live token in its workspace? One read of the
/// `token_revoke` marker, keyed by `(ws, subject)`. `false` if the sub does not parse to a known
/// subject kind (nothing to revoke) or on a store read error (bounded by TTL — see `verify_token`).
async fn is_live_revoked(gw: &Gateway, principal: &Principal) -> bool {
    let Some(subject) = Subject::parse(principal.sub()) else {
        return false;
    };
    token_revoked(&gw.node.store, principal.ws(), &subject)
        .await
        .unwrap_or(false)
}

/// Verify an API-key bearer credential: parse → O(1) ws-scoped lookup → constant-time HMAC compare
/// → status + lazy-expiry → resolve caps → `Principal::for_key`. Every failure is the same opaque
/// `Invalid` (no oracle on whether the key exists / is revoked / is expired / had the wrong secret).
async fn authenticate_apikey(gw: &Gateway, token: &str) -> Result<Principal, AuthRejection> {
    let key = lb_apikey::parse_bearer(token).ok_or(AuthRejection::Invalid)?;
    let principal = lb_host::apikey_authenticate(
        &gw.node.store,
        &gw.node.apikeys,
        &gw.pepper,
        key.ws,
        key.key_id,
        key.secret,
        gw.now,
    )
    .await
    .map_err(|_| AuthRejection::Invalid)?;
    Ok(principal)
}
