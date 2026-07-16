//! `mint_full_session` — the ONE role-correct token-minting path (email-login scope), factored out of
//! `routes/login.rs` so the legacy `/login` and all three `/auth/*` routes mint **byte-identically**.
//! Given a resolved `(principal_sub, workspace)` that has ALREADY passed the membership + credential
//! gates, it builds the viewer-floor claims, unions the durable grant/role caps (`resolve_caps_live`)
//! and the nav-derived reach caps, signs the token, and best-effort registers the workspace in the
//! directory so the switcher lists it. Returns the token + the caps it carries.
//!
//! This is deliberately the same sequence login-hardening + nav-reach shipped — no new issuance
//! behavior, just one home for it. The membership/credential/disabled gates run in the CALLER (they
//! differ per route: `/auth/login` verifies the global password, `/auth/select` a select-token,
//! `/auth/switch` a full token); this function is purely the mint once the caller has decided to.

use std::sync::Arc;

use lb_auth::{mint, SigningKey};
use lb_host::Node;

use super::credentials::dev_claims;

/// The human session lifetime — long enough for a working session, short enough that a leaked token
/// expires. (Was `SESSION_TTL_SECS` in `routes/login.rs`; centralized here so every mint path agrees.)
pub const SESSION_TTL_SECS: u64 = 60 * 60 * 12;

/// The outcome of a mint: the signed token and the caps it carries (surfaced so a route reply can
/// echo them for the UI's cap-gate — a convenience, never the boundary).
pub struct MintedSession {
    pub token: String,
    pub caps: Vec<String>,
}

/// Mint a full workspace session for `principal_sub` in `workspace`. `principal_sub` is the
/// canonical `user:<name>` handle; `now` is the gateway clock. Runs the SAME role-correct issuance as
/// the shipped login: viewer floor ∪ `resolve_caps_live` ∪ nav-reach. Best-effort directory register.
pub async fn mint_full_session(
    node: &Arc<Node>,
    key: &SigningKey,
    principal_sub: &str,
    workspace: &str,
    now: u64,
) -> MintedSession {
    let mut claims = dev_claims(principal_sub, workspace, now, SESSION_TTL_SECS);

    // Fold the DURABLE grant store into the token (authz-grants scope): the token is a cached
    // projection of `resolve_caps`. Grants are stored under the BARE user name, so resolve with the
    // bare handle — `resolve_caps` re-wraps it as `Subject::User`. Best-effort — a store hiccup never
    // fails the mint (the viewer floor still mints a working session).
    let bare_user = principal_sub.strip_prefix("user:").unwrap_or(principal_sub);
    if let Ok(resolved) = lb_host::resolve_caps_live(&node.store, workspace, bare_user).await {
        claims.caps.extend(resolved);
        claims.caps.sort();
        claims.caps.dedup();
    }

    // Fold the subject's nav-derived reach caps (`reach:<surface>:view`) — nav-reach scope. Runs
    // AFTER the grant fold so the resolver strips items against the caller's FULL caps (no widening).
    // Degrade OPEN on a resolve error (reach-all), same posture as the grant fold.
    let reach_principal = lb_auth::Principal::routed(
        principal_sub.to_string(),
        workspace.to_string(),
        claims.caps.clone(),
    );
    let reach = match lb_host::nav_resolve(node, &reach_principal, workspace).await {
        Ok(resolved) => lb_host::reach_caps(&resolved),
        Err(_) => vec![lb_host::REACH_ALL.to_string()],
    };
    claims.caps.extend(reach);
    claims.caps.sort();
    claims.caps.dedup();

    let caps = claims.caps.clone();
    let token = mint(key, &claims);

    // Best-effort: make this workspace listable in the switcher. Never fails the mint. Runs under the
    // just-minted token's verified principal so the directory write carries the real `workspace.create`
    // grant (the token was just signed by this key, so verify never fails).
    if let Ok(self_principal) = lb_auth::verify(key, &token, now) {
        let _ = lb_host::workspace_create(&node.store, &self_principal, workspace, workspace, now)
            .await;
    }

    MintedSession { token, caps }
}
