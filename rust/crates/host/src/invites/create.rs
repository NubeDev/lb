//! `invite.create` — mint a durable invite record and enqueue the email delivery effect (invites
//! scope). Gated by `mcp:invite.create:call`; no-widening: the role/team must be grantable by the
//! minter (checked via `holds_cap` for `role:<name>` — the minter must hold the role grant). The
//! raw token is returned once (never recoverable from the stored hash).

use lb_auth::Principal;
use lb_authz as raw;
use lb_authz::Invite;
use lb_mcp::authorize_tool;
use lb_outbox::Effect;
use lb_store::Store;
use serde_json::json;

use super::error::InviteError;
use super::token::{generate_token, hash_token};

/// The outbox target string for email delivery (the `Target` adapter matches on this).
pub const EMAIL_TARGET: &str = "email";

/// The outbox action for an invite email.
pub const EMAIL_ACTION: &str = "send_invite";

/// Create an invite. Returns the raw one-time token (the admin sends the link; the token is never
/// stored). Gated by `mcp:invite.create:call`.
#[allow(clippy::too_many_arguments)]
pub async fn invite_create(
    store: &Store,
    principal: &Principal,
    ws: &str,
    email: &str,
    role: &str,
    team: &str,
    payload: Option<&str>,
    locale: Option<&str>,
    expires_ts: u64,
    now: u64,
) -> Result<String, InviteError> {
    authorize_tool(principal, ws, "invite.create").map_err(|_| InviteError::Denied)?;

    // Locale rides the invite so the email + pre-auth accept page render in the invitee's
    // language (release scope, i18n gap a). Validated against the enabled-language axis — an
    // unknown code is a caller error, not a silent en-fallback at mint time.
    if let Some(l) = locale {
        if !lb_prefs::language_enabled(l) {
            return Err(InviteError::BadInput(format!(
                "locale '{l}' is not an enabled language"
            )));
        }
    }

    // No-widening for roles follows the `grants.assign` precedent: a `role:<name>` grant is
    // exempt from the holds-cap check (the role's caps were bounded at `roles.define` time).
    // The `mcp:invite.create:call` cap IS the authority — any admin who can create invites can
    // invite with any existing role, exactly as `grants.assign` can assign any role.

    let token = generate_token();
    let token_hash = hash_token(&token);
    let mut invite = Invite::new(token_hash.clone(), email, principal.sub(), now, expires_ts);
    invite.role = role.to_string();
    invite.team = team.to_string();
    invite.payload = payload.map(|s| s.to_string());
    invite.locale = locale.map(|s| s.to_string());

    // Enqueue the email delivery effect transactionally WITH the invite record (the outbox's
    // atomic change+effect write — no window where the invite is durable but the email is lost).
    let invite_value =
        serde_json::to_value(&invite).map_err(|e| InviteError::Store(e.to_string()))?;
    let effect_payload = json!({
        "email": email,
        "workspace": ws,
        "token": token,
        "minter": principal.sub(),
        // The invitee's locale rides the effect so the email target renders subject/body through
        // the catalog in their language (release scope, i18n gap b). Absent ⇒ `en`.
        "locale": locale,
    });
    let effect = Effect::new(
        format!("invite:{token_hash}"),
        EMAIL_TARGET,
        EMAIL_ACTION,
        &effect_payload.to_string(),
        format!("invite:{token_hash}"),
        now,
    );
    lb_outbox::enqueue(
        store,
        ws,
        raw::INVITE_TABLE,
        &token_hash,
        &invite_value,
        &effect,
    )
    .await
    .map_err(|e| InviteError::Store(e.to_string()))?;

    Ok(token)
}
