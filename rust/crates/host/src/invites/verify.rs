//! `invite.verify` — the **pre-auth, read-only** token preview (release scope, i18n gap a). The
//! accept page has no session yet but must render in the invite's language, so this verb exposes
//! the minimum the page needs: the invite's locale, email, and whether it is still redeemable.
//! Token-gated exactly like `invite.accept` (presenting the full-entropy token IS the authority);
//! it mutates nothing and leaks nothing beyond what the accept flow would reveal anyway — no
//! role/team/payload, no minter.

use lb_authz as raw;
use lb_store::Store;
use serde::Serialize;

use super::error::InviteError;
use super::token::{hash_token, validate_token};

/// The pre-auth preview of an invite: what the accept page may know before any session exists.
#[derive(Debug, Clone, Serialize)]
pub struct InvitePreview {
    /// The invitee's email (the accept form pre-fills it).
    pub email: String,
    /// The invite's locale (BCP-47 base code) — the accept page renders in this. `None` ⇒ `en`.
    pub locale: Option<String>,
    /// Whether the invite can still be redeemed at `now`.
    pub redeemable: bool,
}

/// Verify an invite token (pre-auth, read-only). Returns the preview, or the same precise errors
/// the accept path gives for a bad/unknown token.
pub async fn invite_verify(
    store: &Store,
    ws: &str,
    token: &str,
    now: u64,
) -> Result<InvitePreview, InviteError> {
    validate_token(token).ok_or(InviteError::BadToken)?;
    let token_hash = hash_token(token);
    let invite = raw::invite_get_raw(store, ws, &token_hash)
        .await?
        .ok_or(InviteError::NotFound)?;
    Ok(InvitePreview {
        email: invite.email.clone(),
        locale: invite.locale.clone(),
        redeemable: invite.is_redeemable(now),
    })
}
