//! `invite.accept` — the pre-auth atomic onboarding chain (invites scope). The ONE unauthenticated
//! verb (besides `/login` and `/hooks`): verifies the invite token, verifies takeover protection,
//! **claims the redemption atomically**, then creates-or-matches the global identity, sets the
//! credential, joins the workspace, applies grants, and mints the session.
//!
//! Ordering is load-bearing (invites review fix — the accept-race/credential-ordering bug): the
//! redemption is claimed via a store-level conditional CREATE (`invite_mark_accepted_raw`) BEFORE
//! any credential/membership mutation, so of two concurrent accepts the loser is rejected before it
//! can overwrite the winner's password. A failure AFTER the claim releases it
//! (`invite_release_claim_raw`) so the invite returns to `pending` for idempotent retry — never a
//! half-joined member holding a dead invite.
//!
//! Security: the token embeds nothing but entropy (ws/role live server-side). An existing identity
//! must have no credential OR the caller must provide `current_secret` matching it (prevents
//! email-match takeover). The accept is rate-limited at the gateway (the public route's concern).

use lb_auth::{mint, Claims, Role, SigningKey};
use lb_authz as raw;
use lb_authz::Subject;
use lb_store::Store;

use super::error::InviteError;
use super::token::{hash_token, validate_token};
use crate::authz::{ensure_builtin_authz_roles, resolve_caps_live};
use crate::credential::{credential_verify, hash_secret};

/// The session TTL for an invited member's first login (12h, matching the gateway's login route).
const SESSION_TTL_SECS: u64 = 12 * 60 * 60;

/// The result of a successful accept: the session token + the principal info.
#[derive(Debug, Clone)]
pub struct AcceptedInvite {
    pub token: String,
    pub sub: String,
    pub workspace: String,
    pub caps: Vec<String>,
}

/// Accept an invite: verify token → verify takeover protection → **claim redemption (CAS)** →
/// create-or-match identity → set credential → join workspace → apply grants → mint session.
/// The `token` is the raw `lbi_…` invite token. `secret` is the new password to set.
/// `current_secret` is required if the identity already has a credential (prevents email-match
/// takeover).
///
/// This is the **pre-auth** verb — it does NOT require a principal. The `signing_key` is the
/// gateway's key for minting the session token.
pub async fn invite_accept(
    store: &Store,
    signing_key: &SigningKey,
    ws: &str,
    token: &str,
    secret: &str,
    current_secret: Option<&str>,
    now: u64,
) -> Result<AcceptedInvite, InviteError> {
    // 1. Validate + hash the token, look up the invite.
    validate_token(token).ok_or(InviteError::BadToken)?;
    let token_hash = hash_token(token);

    let invite = raw::invite_get_raw(store, ws, &token_hash)
        .await?
        .ok_or(InviteError::NotFound)?;

    if !invite.is_redeemable(now) {
        return Err(not_redeemable_error(&invite, now));
    }

    if secret.is_empty() {
        return Err(InviteError::BadToken);
    }

    // 2. Takeover protection — READS ONLY, nothing mutated yet. The sub is `user:<email>`.
    let sub = format!("user:{}", invite.email);
    let identity_exists = raw::identity_get(store, &sub).await?.is_some();
    if identity_exists {
        // Existing identity — must verify the current credential (prevents email-match takeover).
        match credential_verify(store, ws, &sub, "").await {
            Ok(crate::credential::CredentialCheck::Absent) => {
                // No credential set — safe to proceed (first-time onboarding for an existing identity).
            }
            Ok(_) => {
                // A credential exists — the caller must provide it.
                let current = current_secret.ok_or(InviteError::IdentityExists(
                    "this email already has an account; provide current_secret".into(),
                ))?;
                match credential_verify(store, ws, &sub, current).await {
                    Ok(crate::credential::CredentialCheck::Ok) => {}
                    _ => {
                        return Err(InviteError::IdentityExists(
                            "current_secret does not match".into(),
                        ));
                    }
                }
            }
            Err(e) => {
                return Err(InviteError::Store(e.to_string()));
            }
        }
    }

    // 3. CLAIM the redemption FIRST (the atomic conditional CREATE — exactly one accept ever gets
    //    `true`). A concurrent loser is rejected HERE, before any credential/membership mutation,
    //    so it can never overwrite the winner's password (the review's accept-race fix).
    let claimed = raw::invite_mark_accepted_raw(store, ws, &token_hash, &sub, now).await?;
    if !claimed {
        // Re-read for the precise rejection (another accept won, or it was revoked in between).
        let err = match raw::invite_get_raw(store, ws, &token_hash).await? {
            Some(i) if !i.is_redeemable(now) => not_redeemable_error(&i, now),
            Some(_) | None => InviteError::AlreadyAccepted,
        };
        return Err(err);
    }

    // 4. Winner only: run the onboarding mutations. On ANY failure, release the claim so the
    //    invite returns to `pending` (idempotent retry — never a dead invite + half-joined member).
    if let Err(e) = onboard(store, ws, &invite, &sub, identity_exists, secret, now).await {
        let _ = raw::invite_release_claim_raw(store, ws, &token_hash, &sub).await;
        return Err(e);
    }

    // 5. Resolve caps and mint the session token.
    let bare = sub.strip_prefix("user:").unwrap_or(&sub);
    let caps = resolve_caps_live(store, ws, bare).await?;
    let claims = Claims {
        sub: sub.clone(),
        ws: ws.to_string(),
        role: Role::Member,
        caps: caps.clone(),
        iat: now,
        exp: now + SESSION_TTL_SECS,
        constraint: None,
        run_id: None,
    };
    let token = mint(signing_key, &claims);

    Ok(AcceptedInvite {
        token,
        sub,
        workspace: ws.to_string(),
        caps,
    })
}

/// Map a non-redeemable invite to its precise rejection.
fn not_redeemable_error(invite: &raw::Invite, now: u64) -> InviteError {
    match invite.status {
        raw::InviteStatus::Accepted => InviteError::AlreadyAccepted,
        raw::InviteStatus::Revoked => InviteError::Revoked,
        raw::InviteStatus::Expired | _ if invite.is_expired(now) => InviteError::Expired,
        _ => InviteError::NotFound,
    }
}

/// The post-claim onboarding mutations: identity + credential + membership + grants. Every step is
/// an idempotent upsert, so a retry after a released claim re-applies safely. Split out so the
/// caller has ONE rollback site (release the claim on `Err`).
async fn onboard(
    store: &Store,
    ws: &str,
    invite: &raw::Invite,
    sub: &str,
    identity_exists: bool,
    secret: &str,
    now: u64,
) -> Result<(), InviteError> {
    // Create the global identity if it did not exist (checked pre-claim, before any mutation).
    if !identity_exists {
        raw::identity_create(store, sub, Some(&invite.email), now).await?;
    }

    // Set the credential (hash + write). Stored in the workspace namespace (the hard wall).
    let phc = hash_secret(secret).map_err(InviteError::Store)?;
    let cred = serde_json::json!({
        "sub": sub,
        "kind": "credential",
        "phc": phc,
        "set_ts": now,
    });
    lb_store::write(store, ws, "credential", sub, &cred).await?;

    // Ensure built-in roles exist, then join the workspace (membership + role:member grant).
    ensure_builtin_authz_roles(store, ws).await?;
    raw::membership_add_raw(store, ws, sub, now).await?;
    let bare = sub.strip_prefix("user:").unwrap_or(sub);
    raw::grant_assign_scoped(
        store,
        ws,
        &Subject::User(bare.to_string()),
        "role:member",
        &raw::Scope::All,
    )
    .await?;

    // Apply the invite's role/team grants (system-grant, not gated — the invite is the authority).
    if !invite.role.is_empty() {
        let role_cap = format!("role:{}", invite.role);
        raw::grant_assign_scoped(
            store,
            ws,
            &Subject::User(bare.to_string()),
            &role_cap,
            &raw::Scope::All,
        )
        .await?;
    }
    if !invite.team.is_empty() {
        // Team membership is via the lb_assets member edge.
        let _ = lb_assets::relate(store, ws, "member", &invite.team, bare).await;
    }
    Ok(())
}
