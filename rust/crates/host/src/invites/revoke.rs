//! `invite.revoke` and `invite.resend` (invites scope). Both gated by `mcp:invite.create:call`
//! (the same mutate cap — revoking/resending is an admin action on the invite record).

use lb_auth::Principal;
use lb_authz as raw;
use lb_outbox::Effect;
use lb_store::Store;
use serde_json::json;

use super::error::InviteError;
use super::token::{generate_token, hash_token};
use lb_mcp::authorize_tool;

/// Revoke a pending invite by its token hash. Idempotent (revoking an already-revoked/accepted
/// invite is a no-op success). Gated by `mcp:invite.create:call`.
pub async fn invite_revoke(
    store: &Store,
    principal: &Principal,
    ws: &str,
    token_hash: &str,
) -> Result<bool, InviteError> {
    authorize_tool(principal, ws, "invite.create").map_err(|_| InviteError::Denied)?;
    Ok(raw::invite_revoke_raw(store, ws, token_hash).await?)
}

/// Resend a pending invite: rotates the token (new hash, new link), keeps the record. The old
/// token is dead (the old hash record is replaced). Gated by `mcp:invite.create:call`.
/// Returns the new raw one-time token.
pub async fn invite_resend(
    store: &Store,
    principal: &Principal,
    ws: &str,
    token_hash: &str,
    now: u64,
) -> Result<String, InviteError> {
    authorize_tool(principal, ws, "invite.create").map_err(|_| InviteError::Denied)?;

    let mut invite = raw::invite_get_raw(store, ws, token_hash)
        .await?
        .ok_or(InviteError::NotFound)?;

    if invite.status != raw::InviteStatus::Pending {
        return Err(InviteError::AlreadyAccepted);
    }

    // Rotate the token: write the invite under the new hash (the old record stays as a revoked
    // tombstone — the old link is dead). The new record + email effect are written atomically.
    let new_token = generate_token();
    let new_hash = hash_token(&new_token);
    raw::invite_revoke_raw(store, ws, token_hash).await?;
    invite.token_hash = new_hash.clone();
    let invite_value =
        serde_json::to_value(&invite).map_err(|e| InviteError::Store(e.to_string()))?;
    let effect_payload = json!({
        "email": invite.email,
        "workspace": ws,
        "token": new_token,
        "minter": principal.sub(),
    });
    let effect = Effect::new(
        format!("invite:{new_hash}"),
        super::create::EMAIL_TARGET,
        super::create::EMAIL_ACTION,
        &effect_payload.to_string(),
        format!("invite:{new_hash}"),
        now,
    );
    lb_outbox::enqueue(
        store,
        ws,
        raw::INVITE_TABLE,
        &new_hash,
        &invite_value,
        &effect,
    )
    .await
    .map_err(|e| InviteError::Store(e.to_string()))?;

    Ok(new_token)
}
