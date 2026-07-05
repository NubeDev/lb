//! The inbound auth resolver (webhooks scope). The gateway route captures the **raw body** (so
//! HMAC verify runs over the exact received bytes) and calls [`webhook_resolve`], which:
//!
//! 1. Loads `webhook:{ws}:{id}` from the workspace namespace. 404/disabled → opaque `NotFound`
//!    (the public route collapses this to a bare `404`/`410` so the endpoint is not a webhook-id
//!    oracle — every unknown/disabled/wrong-ws id looks the same).
//! 2. Verifies per `auth_mode`:
//!    - `bearer` → the request's `Authorization: Bearer lbk_{ws}.{keyid}.{secret}` header. We
//!      route through `apikey_authenticate` (the cached, constant-time-HMAC verify path). The
//!      resulting `Principal` carries the hook's caps (`mcp:ingest.write:call`) — exactly what
//!      `ingest.write` re-checks. The bearer's ws MUST match the URL ws (the apikey path already
//!      enforces the namespace wall, so a ws-B bearer can never authenticate against a ws-A hook).
//!    - `signature` → the request's admin-picked header (e.g. `X-Signature: sha256=…`). We
//!      mediate-read the shared secret from `lb-secrets` (Workspace visibility → host-readable)
//!      and run `verify_signature` over the raw body (constant-time compare). On success we build
//!      a synthetic `Principal::for_key("webhook:{id}", ws, [INGEST_CAP])` — a host-built principal
//!      for a verified delivery, scoped to exactly the cap the route needs.
//! 3. Returns `(record, principal)` so the route can build the Sample over `record.series` and
//!    hand both to `webhook_accept`.
//!
//! Every failure collapses to the same opaque [`WebhookError::NotFound`]/`Invalid` (no oracle on
//! whether the hook exists, is revoked, or had the wrong secret). The secret never reaches an
//! error message.

use lb_auth::Principal;
use lb_store::{read, Store};

use super::error::WebhookError;
use super::model::{AuthMode, WebhookRecord, TABLE};
use super::verify::verify_signature;
use super::ApiKeyCache;

/// The cap the synthetic `signature`-mode principal resolves to (mirrors the `bearer`-mode
/// apikey's narrowed cap, so the downstream `ingest.write` gate passes identically across modes).
pub const INGEST_CAP: &str = "mcp:ingest.write:call";

/// Resolve + verify an inbound hit at `/hooks/{ws}/{id}`: load the record, verify the credential
/// per mode, and return the verified record + principal. `body` is the **raw received bytes** (the
/// caller MUST capture before any JSON parse — the HMAC is over those exact bytes).
/// `bearer_value` is the request's `Authorization` header value, if present.
/// `header_lookup` resolves the admin-picked signature header by NAME (the route does not know
/// `record.hmac_header` ahead of time — it passes a closure over its header map, and the host
/// reads the exact header the record names). Returns `None` from the closure for an absent header.
#[allow(clippy::too_many_arguments)]
pub async fn webhook_resolve(
    store: &Store,
    cache: &ApiKeyCache,
    pepper: &[u8],
    ws: &str,
    id: &str,
    body: &[u8],
    bearer_value: Option<&str>,
    header_lookup: impl Fn(&str) -> Option<String>,
    now: u64,
) -> Result<(WebhookRecord, Principal), WebhookError> {
    // 1. O(1) ws-scoped lookup. A webhook minted in another workspace simply is not in this
    //    namespace (the wall is the namespace). 404 is opaque — same shape as a wrong id.
    let value = read(store, ws, TABLE, id)
        .await?
        .ok_or(WebhookError::NotFound)?;
    let record: WebhookRecord = serde_json::from_value(value).map_err(unexpected)?;

    // 2. A revoked hook is gone (the tombstone). 410 in the route; opaque here.
    if record.is_revoked() {
        return Err(WebhookError::Revoked);
    }

    // 3. Per-mode verify.
    let principal = match record.auth_mode {
        AuthMode::Bearer => {
            verify_bearer(store, cache, pepper, ws, &record, bearer_value, now).await?
        }
        AuthMode::Signature => {
            verify_signature_mode(store, ws, &record, body, &header_lookup).await?
        }
    };

    Ok((record, principal))
}

/// `bearer` mode: pull the `lbk_…` token out of the `Authorization: Bearer …` header value and
/// route through the cached apikey verify path. The bearer's ws matches the URL ws because the
/// apikey row lives in that workspace's namespace (a ws-B bearer can't resolve in ws-A).
///
/// **Linkage check (load-bearing):** the presented keyid MUST match `record.bearer_key_id` — a
/// webhook is authenticated by ITS issued key, not any workspace apikey that happens to resolve to
/// `mcp:ingest.write:call`. This keeps a leaked sibling key from impersonating the hook and pins
/// revoke to the one linked credential (rotate replaces THIS id; revoke kills THIS id).
async fn verify_bearer(
    store: &Store,
    cache: &ApiKeyCache,
    pepper: &[u8],
    ws: &str,
    record: &WebhookRecord,
    bearer_value: Option<&str>,
    now: u64,
) -> Result<Principal, WebhookError> {
    let expected_key_id = record.bearer_key_id.as_deref().ok_or_else(|| {
        WebhookError::Store(lb_store::StoreError::Decode(format!(
            "bearer-mode webhook {} has no bearer_key_id",
            record.id
        )))
    })?;
    let raw = bearer_value.ok_or(WebhookError::Invalid)?;
    let token = raw
        .strip_prefix("Bearer ")
        .or_else(|| raw.strip_prefix("bearer "))
        .ok_or(WebhookError::Invalid)?
        .trim();
    if !token.starts_with("lbk_") {
        return Err(WebhookError::Invalid);
    }
    let key = lb_apikey::parse_bearer(token).ok_or(WebhookError::Invalid)?;
    // The bearer's ws MUST match the URL ws — defense in depth on top of the apikey namespace wall
    // (a ws-B bearer posted to /hooks/wsA/... is refused here even if its row somehow resolved).
    if key.ws != ws {
        return Err(WebhookError::Invalid);
    }
    // The linkage check: the presented keyid must be the one issued for THIS hook. A different
    // workspace apikey — even a valid one holding `mcp:ingest.write:call` — does not authenticate
    // this webhook (rule: one hook, one credential).
    if key.key_id != expected_key_id {
        return Err(WebhookError::Invalid);
    }
    let principal =
        crate::apikey_authenticate(store, cache, pepper, key.ws, key.key_id, key.secret, now)
            .await
            .map_err(|_| WebhookError::Invalid)?;
    // The principal must actually hold the ingest cap (a revoked-grant apikey resolves to no caps
    // → refuse). This is the load-bearing re-check on top of "the apikey verified."
    if !crate::authz::holds_cap(&principal, ws, INGEST_CAP) {
        return Err(WebhookError::Invalid);
    }
    Ok(principal)
}

/// `signature` mode: mediate-read the shared secret from `lb-secrets` (Workspace visibility →
/// host-readable), then constant-time-verify the HMAC over the raw body. `header_lookup` resolves
/// the admin-picked header by NAME (the route passes its header map; the host reads the exact
/// header the record names).
async fn verify_signature_mode(
    store: &Store,
    ws: &str,
    record: &WebhookRecord,
    body: &[u8],
    header_lookup: &impl Fn(&str) -> Option<String>,
) -> Result<Principal, WebhookError> {
    let secret_ref = record.secret_ref.as_deref().ok_or_else(|| {
        WebhookError::Store(lb_store::StoreError::Decode(format!(
            "signature-mode webhook {} has no secret_ref",
            record.id
        )))
    })?;
    // Host-mediated read of a Workspace-visibility secret. The wall holds: gate 1 (workspace) by
    // `ws`, and only a Workspace secret resolves (a Private one returns Denied → we treat as
    // Invalid, never leaking the value).
    let shared = lb_secrets::get_workspace(store, ws, secret_ref)
        .await
        .map_err(|_| WebhookError::Invalid)?;
    // Resolve the admin-picked signature header value (constant-time compare on the value).
    let header_value = header_lookup(&record.hmac_header);
    // Constant-time HMAC verify over the EXACT raw bytes. Every failure is the same opaque error.
    verify_signature(shared.as_bytes(), body, header_value.as_deref())
        .map_err(|_| WebhookError::Invalid)?;
    // Build the synthetic webhook principal: host-built (verified delivery), scoped to the single
    // cap the route needs. `Principal::for_key` is the dedicated constructor for a non-human,
    // host-resolved subject (NOT the co-trust `routed` path).
    let sub = format!("webhook:{}", record.id);
    Ok(Principal::for_key(
        sub,
        ws.to_string(),
        vec![INGEST_CAP.to_string()],
    ))
}

fn unexpected(e: serde_json::Error) -> WebhookError {
    WebhookError::Store(lb_store::StoreError::Decode(e.to_string()))
}
