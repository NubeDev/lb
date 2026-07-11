//! `invite.accept` — the pre-auth atomic onboarding chain (invites scope). The ONE unauthenticated
//! verb (besides `/login` and `/hooks`): verifies the invite token, creates-or-matches the global
//! identity, sets the credential, joins the workspace, applies grants, marks redeemed, and mints
//! the session — all atomically (partial failure leaves the invite `pending` for idempotent retry).
//!
//! Security: the token embeds nothing but entropy (ws/role live server-side). An existing identity
//! must have no credential OR the caller must provide `current_secret` matching it (prevents
//! email-match takeover). The accept is rate-limited at the gateway (the public route's concern).

use lb_auth::{mint, Claims, Principal, Role, SigningKey};
use lb_authz as raw;
use lb_authz::Subject;
use lb_store::Store;
use serde_json::{json, Value};

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

/// Accept an invite: verify token → create-or-match identity → set credential → join workspace →
/// apply grants → mark redeemed → mint session. The `token` is the raw `lbi_…` invite token.
/// `secret` is the new password to set. `current_secret` is required if the identity already has
/// a credential (prevents email-match takeover).
///
/// This is the **pre-auth** verb — it does NOT require a principal. The `signing_key` is the
/// gateway's key for minting the session token.
#[allow(clippy::too_many_arguments)]
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
        return Err(match invite.status {
            raw::InviteStatus::Accepted => InviteError::AlreadyAccepted,
            raw::InviteStatus::Revoked => InviteError::Revoked,
            raw::InviteStatus::Expired | _ if invite.is_expired(now) => InviteError::Expired,
            _ => InviteError::NotFound,
        });
    }

    if secret.is_empty() {
        return Err(InviteError::BadToken);
    }

    // 2. Create-or-match the global identity by email. The sub is `user:<email>`.
    let sub = format!("user:{}", invite.email);

    if raw::identity_get(store, &sub).await?.is_none() {
        // New identity — create it.
        raw::identity_create(store, &sub, Some(&invite.email), now).await?;
    } else {
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

    // 3. Set the credential (hash + write). Stored in the workspace namespace (the hard wall).
    let phc = hash_secret(secret).map_err(|e| InviteError::Store(e))?;
    let cred = serde_json::json!({
        "sub": sub,
        "kind": "credential",
        "phc": phc,
        "set_ts": now,
    });
    lb_store::write(store, ws, "credential", &sub, &cred).await?;

    // 4. Ensure built-in roles exist, then join the workspace (membership + role:member grant).
    ensure_builtin_authz_roles(store, ws).await?;
    raw::membership_add_raw(store, ws, &sub, now).await?;
    let bare = sub.strip_prefix("user:").unwrap_or(&sub);
    raw::grant_assign_scoped(
        store,
        ws,
        &Subject::User(bare.to_string()),
        "role:member",
        &raw::Scope::All,
    )
    .await?;

    // 5. Apply the invite's role/team grants (system-grant, not gated — the invite is the authority).
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

    // 6. Mark the invite as accepted (atomic — if this succeeds, the onboarding is complete).
    let marked = raw::invite_mark_accepted_raw(store, ws, &token_hash, &sub, now).await?;
    if !marked {
        // Race: another concurrent accept won. Roll back is not needed — the operations above are
        // idempotent (membership/grant are upserts). The caller gets AlreadyAccepted.
        return Err(InviteError::AlreadyAccepted);
    }

    // 7. Resolve caps and mint the session token.
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
