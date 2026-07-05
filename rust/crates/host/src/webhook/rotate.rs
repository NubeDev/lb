//! `webhook.rotate` — replace a webhook's credential with a fresh one, killing the old credential
//! instantly (webhooks scope, decision: INSTANT rotation, no grace/overlap — mirroring
//! `apikey.rotate`). Gated by `mcp:webhook.manage:call`, workspace-first.
//!
//! - `bearer` mode delegates to `apikey_rotate` (fresh secret, same keyid + grants, old hash dead,
//!   cache busted). The new `lbk_…` string is returned once.
//! - `signature` mode overwrites the `lb-secrets` shared secret at `webhook/{id}` with a fresh
//!   one (under the creator's authority — `secret:webhook/*:write` is re-checked by `set_with`).
//!   The old secret dies the moment the row upserts; no overlap. The new shared secret is returned
//!   once.
//!
//! The webhook record itself is untouched (same URL/series/name); only the credential rolls. A
//! revoked webhook refuses rotation (no reviving a dead hook).

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_secrets::{set_with, Visibility};
use lb_store::{read, Store};

use super::error::WebhookError;
use super::model::{AuthMode, WebhookRecord, TABLE};
use super::secret::generate_shared_secret;
use super::ApiKeyCache;

/// Rotate webhook `id`'s credential in `ws` as `principal`, returning the one-time new credential
/// (the `lbk_…` bearer in `bearer` mode, the shared secret in `signature` mode).
pub async fn webhook_rotate(
    store: &Store,
    cache: &ApiKeyCache,
    principal: &Principal,
    ws: &str,
    id: &str,
    pepper: &[u8],
) -> Result<String, WebhookError> {
    authorize_tool(principal, ws, "webhook.manage").map_err(|_| WebhookError::Denied)?;

    let value = read(store, ws, TABLE, id)
        .await?
        .ok_or(WebhookError::NotFound)?;
    let record: WebhookRecord = serde_json::from_value(value).map_err(unexpected)?;
    if record.is_revoked() {
        return Err(WebhookError::Revoked);
    }

    match record.auth_mode {
        AuthMode::Bearer => rotate_bearer(store, cache, principal, ws, &record, pepper).await,
        AuthMode::Signature => rotate_signature(store, principal, ws, &record).await,
    }
}

/// `bearer` mode: delegate to `apikey_rotate` (same keyid, fresh secret, hash replaced, cache
/// busted — old secret dead on the next request on this node).
async fn rotate_bearer(
    store: &Store,
    cache: &ApiKeyCache,
    principal: &Principal,
    ws: &str,
    record: &WebhookRecord,
    pepper: &[u8],
) -> Result<String, WebhookError> {
    let key_id = record.bearer_key_id.as_deref().ok_or_else(|| {
        WebhookError::Store(lb_store::StoreError::Decode(format!(
            "bearer-mode webhook {} has no bearer_key_id",
            record.id
        )))
    })?;
    let bearer = crate::apikey_rotate(store, cache, principal, ws, key_id, pepper)
        .await
        .map_err(map_apikey_err)?;
    Ok(bearer)
}

/// `signature` mode: overwrite the shared secret in `lb-secrets` at `webhook/{id}`. The creator
/// must hold `secret:webhook/*:write` (re-checked by `set_with` — the no-widening guard for the
/// secret surface). Idempotent overwrite on the creator's own record (host-stamped owner).
async fn rotate_signature(
    store: &Store,
    principal: &Principal,
    ws: &str,
    record: &WebhookRecord,
) -> Result<String, WebhookError> {
    let secret_ref = record.secret_ref.as_deref().ok_or_else(|| {
        WebhookError::Store(lb_store::StoreError::Decode(format!(
            "signature-mode webhook {} has no secret_ref",
            record.id
        )))
    })?;
    let fresh = generate_shared_secret();
    set_with(
        store,
        principal,
        ws,
        secret_ref,
        &fresh,
        Visibility::Workspace,
    )
    .await
    .map_err(|_| WebhookError::Denied)?;
    Ok(fresh)
}

fn unexpected(e: serde_json::Error) -> WebhookError {
    WebhookError::Store(lb_store::StoreError::Decode(e.to_string()))
}

/// Map an `ApiKeyError` from the linked `apikey_rotate` onto the webhook error surface.
fn map_apikey_err(e: crate::ApiKeyError) -> WebhookError {
    use crate::ApiKeyError;
    match e {
        ApiKeyError::Denied => WebhookError::Denied,
        ApiKeyError::NotFound => WebhookError::NotFound,
        ApiKeyError::Store(s) => WebhookError::Store(s),
        ApiKeyError::Widen(c) => WebhookError::Widen(c),
        ApiKeyError::BadInput(m) => WebhookError::BadInput(m),
        ApiKeyError::Revoked | ApiKeyError::Expired | ApiKeyError::Invalid => WebhookError::Revoked,
    }
}
