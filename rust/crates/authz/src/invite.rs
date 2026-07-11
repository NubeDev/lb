//! **Invites** — durable, revocable, single-use token records for onboarding people who don't
//! exist yet (invites scope). An admin mints an invite carrying role/team intent + an opaque
//! payload; an outbox effect delivers the email; a pre-auth accept route redeems the token into
//! `identity` + `membership` + grants — atomically, with caps live on first login.
//!
//! The record is workspace-scoped (`invite:{ws}:{hash}`). The token embeds nothing but entropy —
//! ws/role/payload live server-side (the scope's security model). The `token_hash` is a SHA-256 of
//! the raw token (the token is 32 random bytes — full entropy, so a fast hash is correct, same
//! reasoning as apikeys). Status: `pending → accepted | revoked | expired`.

use lb_store::{create, delete, list as store_list, read, write, Store, StoreError};
use serde::{Deserialize, Serialize};

/// The store table invites live in, within a workspace namespace.
pub const INVITE_TABLE: &str = "invite";

/// The constant `kind` discriminant so [`invite_list_raw`] can equality-filter every invite row.
pub const INVITE_KIND: &str = "invite";

/// The invite status lifecycle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum InviteStatus {
    #[default]
    Pending,
    Accepted,
    Revoked,
    Expired,
}

/// A durable invite record (invites scope). Workspace-scoped; the `token_hash` is the SHA-256 of
/// the raw invite token (the record id is `invite:{token_hash}` for O(1) lookup on accept).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Invite {
    /// The token hash (SHA-256 of the raw `lbi_…` token). Also the record id suffix.
    pub token_hash: String,
    /// The invitee's email (the person who doesn't have an account yet).
    pub email: String,
    /// The role to grant on join (e.g. `member`, `viewer`). Empty = none.
    #[serde(default)]
    pub role: String,
    /// The team to grant on join (e.g. `guardians`). Empty = none.
    #[serde(default)]
    pub team: String,
    /// Opaque caller payload (e.g. an extension's guardian-record id). Core never interprets it
    /// (rule 10).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<String>,
    /// The invitee's locale (BCP-47 base code, e.g. `es`) — set at mint time so the invite email
    /// and the pre-auth accept page render in the invitee's language, and copied into the new
    /// member's `language` pref on accept (release scope, i18n gap a). Additive serde-default:
    /// `None` on old records ⇒ the `en` fallback.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    /// The lifecycle status.
    pub status: InviteStatus,
    /// Who minted the invite (their sub).
    pub minter: String,
    /// Logical ts the invite was created.
    pub created_ts: u64,
    /// Expiry ts (0 = never). An accept after this is rejected.
    pub expires_ts: u64,
    /// Who redeemed the invite (their sub). Set on accept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepted_by: Option<String>,
    /// When the invite was redeemed. Set on accept.
    #[serde(default)]
    pub accepted_ts: u64,
    /// Constant discriminant so `invite_list_raw` selects every row.
    pub kind: String,
}

impl Invite {
    pub fn new(
        token_hash: impl Into<String>,
        email: impl Into<String>,
        minter: impl Into<String>,
        created_ts: u64,
        expires_ts: u64,
    ) -> Self {
        Self {
            token_hash: token_hash.into(),
            email: email.into(),
            role: String::new(),
            team: String::new(),
            payload: None,
            locale: None,
            status: InviteStatus::Pending,
            minter: minter.into(),
            created_ts,
            expires_ts,
            accepted_by: None,
            accepted_ts: 0,
            kind: INVITE_KIND.to_string(),
        }
    }

    /// True if the invite is expired at `now` (and has a non-zero expiry).
    pub fn is_expired(&self, now: u64) -> bool {
        self.expires_ts > 0 && now >= self.expires_ts
    }

    /// True if the invite is redeemable: pending and not expired.
    pub fn is_redeemable(&self, now: u64) -> bool {
        self.status == InviteStatus::Pending && !self.is_expired(now)
    }
}

/// Record id for an invite: `invite:{token_hash}` (O(1) point read on accept).
pub(crate) fn invite_id(token_hash: &str) -> String {
    token_hash.to_string()
}

/// Create (mint) an invite record. Raw store verb — the host service gates it.
pub async fn invite_create_raw(store: &Store, ws: &str, invite: &Invite) -> Result<(), StoreError> {
    let value = serde_json::to_value(invite).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(
        store,
        ws,
        INVITE_TABLE,
        &invite_id(&invite.token_hash),
        &value,
    )
    .await
}

/// Read an invite by its token hash. Raw store verb.
pub async fn invite_get_raw(
    store: &Store,
    ws: &str,
    token_hash: &str,
) -> Result<Option<Invite>, StoreError> {
    match read(store, ws, INVITE_TABLE, &invite_id(token_hash)).await? {
        Some(v) => {
            let invite: Invite =
                serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string()))?;
            Ok(Some(invite))
        }
        None => Ok(None),
    }
}

/// List all invites in workspace `ws`. Raw store verb. Sorted by `created_ts` (newest first via
/// reverse in the caller — the store returns insertion order).
pub async fn invite_list_raw(store: &Store, ws: &str) -> Result<Vec<Invite>, StoreError> {
    let rows = store_list(store, ws, INVITE_TABLE, "kind", INVITE_KIND).await?;
    rows.into_iter()
        .map(|v| serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string())))
        .collect()
}

/// Mark an invite as revoked (tombstone-style upsert — idempotent). Raw store verb.
pub async fn invite_revoke_raw(
    store: &Store,
    ws: &str,
    token_hash: &str,
) -> Result<bool, StoreError> {
    let Some(mut invite) = invite_get_raw(store, ws, token_hash).await? else {
        return Ok(false);
    };
    if invite.status != InviteStatus::Pending {
        return Ok(false);
    }
    invite.status = InviteStatus::Revoked;
    let value = serde_json::to_value(&invite).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, INVITE_TABLE, &invite_id(token_hash), &value).await?;
    Ok(true)
}

/// The table the atomic redemption claims live in. A claim is a `CREATE`-only row keyed by the
/// token hash: SurrealDB's `CREATE` errors on a duplicate id, so the FIRST claimer binds and every
/// concurrent loser gets [`StoreError::Conflict`] — the same first-settle primitive the agent's
/// Ask decision uses (`lb_store::create`). This is what makes double-redeem lose **before** any
/// credential/membership mutation (invites review fix: the plain read-modify-write mark let two
/// concurrent accepts both pass, the loser overwriting the winner's password).
pub const INVITE_CLAIM_TABLE: &str = "invite_claim";

/// Mark an invite as accepted by `sub` at `now`. Raw store verb. Returns `false` if the invite is
/// not redeemable (already accepted/revoked/expired) **or** if another accept already claimed the
/// redemption — the claim is a store-level conditional `CREATE` (first write binds, `Conflict`
/// for everyone else), so exactly ONE caller ever sees `true`. The caller must invoke this
/// **before** any credential/membership mutation and only mutate on `true`.
pub async fn invite_mark_accepted_raw(
    store: &Store,
    ws: &str,
    token_hash: &str,
    sub: &str,
    now: u64,
) -> Result<bool, StoreError> {
    let Some(mut invite) = invite_get_raw(store, ws, token_hash).await? else {
        return Ok(false);
    };
    if !invite.is_redeemable(now) {
        return Ok(false);
    }
    // The atomic claim: CREATE binds the first caller, Conflict rejects every racer. Nothing has
    // been mutated for the loser at this point — it never proceeds past here.
    let claim = serde_json::json!({ "sub": sub, "ts": now, "kind": "invite_claim" });
    match create(
        store,
        ws,
        INVITE_CLAIM_TABLE,
        &invite_id(token_hash),
        &claim,
    )
    .await
    {
        Ok(()) => {}
        Err(StoreError::Conflict) => return Ok(false),
        Err(e) => return Err(e),
    }
    // Winner only from here: reflect the claim on the invite record itself.
    invite.status = InviteStatus::Accepted;
    invite.accepted_by = Some(sub.to_string());
    invite.accepted_ts = now;
    let value = serde_json::to_value(&invite).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, INVITE_TABLE, &invite_id(token_hash), &value).await?;
    Ok(true)
}

/// Release a redemption claim held by `sub` — the winner's **rollback** when a post-claim
/// onboarding step fails (the invite returns to `pending` so the invitee can retry, per the scope's
/// "partial failure leaves the invite `pending`"). Only the claim's own `sub` may release it, and
/// only while the invite is `accepted` — so a racer can never un-claim the winner.
pub async fn invite_release_claim_raw(
    store: &Store,
    ws: &str,
    token_hash: &str,
    sub: &str,
) -> Result<(), StoreError> {
    let Some(mut invite) = invite_get_raw(store, ws, token_hash).await? else {
        return Ok(());
    };
    if invite.status != InviteStatus::Accepted || invite.accepted_by.as_deref() != Some(sub) {
        return Ok(());
    }
    invite.status = InviteStatus::Pending;
    invite.accepted_by = None;
    invite.accepted_ts = 0;
    let value = serde_json::to_value(&invite).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, INVITE_TABLE, &invite_id(token_hash), &value).await?;
    delete(store, ws, INVITE_CLAIM_TABLE, &invite_id(token_hash)).await?;
    Ok(())
}
