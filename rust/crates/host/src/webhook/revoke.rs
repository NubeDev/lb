//! `webhook.revoke` — tombstone a webhook + revoke its linked apikey (bearer) / wipe its shared
//! secret (signature) + cache-bust (webhooks scope). Gated by `mcp:webhook.manage:call`,
//! workspace-first. The tombstone is a `status = "__revoked__"` upsert — **idempotent** (re-revoking
//! writes the same tombstone harmlessly), so it replays cleanly under sync (§6.8) like the apikey
//! tombstone.
//!
//! In `bearer` mode the linked apikey is revoked too (via `apikey_revoke`): its cache entry is
//! busted so the very next inbound request **on this node** refuses (instant local revoke — the
//! mandatory integration property). In `signature` mode the shared secret is left in `lb-secrets`
//! (the tombstone alone gates verify); a future hardening can wipe it, but the wall is the record
//! status, not secret absence — keeping the secret lets a `rotate` revive the hook if that ever
//! becomes a verb (it is not in v1: revoke is terminal).

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_store::{read, write, Store};

use super::error::WebhookError;
use super::model::{AuthMode, WebhookRecord, TABLE, TOMBSTONE_STATUS};
use super::ApiKeyCache;

/// Revoke webhook `id` in `ws` as `principal`. Idempotent. `cache` is the node's shared apikey
/// verification cache (busted for the linked apikey in `bearer` mode so the next auth on this node
/// misses and reads the tombstone).
pub async fn webhook_revoke(
    store: &Store,
    cache: &ApiKeyCache,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<(), WebhookError> {
    authorize_tool(principal, ws, "webhook.manage").map_err(|_| WebhookError::Denied)?;

    // Read-modify-write to flip status → tombstone (preserves name/auth_mode for the list/audit
    // view). Idempotent: re-revoking an already-tombstoned record writes the same status back.
    let value = read(store, ws, TABLE, id)
        .await?
        .ok_or(WebhookError::NotFound)?;
    let mut record: WebhookRecord = serde_json::from_value(value).map_err(unexpected)?;

    // Revoke the linked apikey FIRST (bearer mode): bust the cache so the very next request that
    // presents the old bearer is refused, BEFORE the tombstone is even observed. Idempotent.
    if record.auth_mode == AuthMode::Bearer {
        if let Some(key_id) = record.bearer_key_id.as_deref() {
            let p = principal.clone();
            crate::apikey_revoke(store, cache, &p, ws, key_id)
                .await
                .map_err(map_apikey_err)?;
        }
    }

    record.status = TOMBSTONE_STATUS.to_string();
    let value =
        serde_json::to_value(&record).map_err(|e| lb_store::StoreError::Decode(e.to_string()))?;
    write(store, ws, TABLE, id, &value).await?;
    Ok(())
}

fn unexpected(e: serde_json::Error) -> WebhookError {
    WebhookError::Store(lb_store::StoreError::Decode(e.to_string()))
}

/// Map an `ApiKeyError` from the linked `apikey_revoke` onto the webhook error surface.
fn map_apikey_err(e: crate::ApiKeyError) -> WebhookError {
    use crate::ApiKeyError;
    match e {
        ApiKeyError::Denied => WebhookError::Denied,
        ApiKeyError::NotFound => WebhookError::NotFound,
        ApiKeyError::Store(s) => WebhookError::Store(s),
        ApiKeyError::Widen(c) => WebhookError::Widen(c),
        ApiKeyError::BadInput(m) => WebhookError::BadInput(m),
        ApiKeyError::Revoked | ApiKeyError::Expired | ApiKeyError::Invalid => WebhookError::Denied,
    }
}
