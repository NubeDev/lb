//! Host entry points over the resolver that bake in [`LiveBuiltinRoleCaps`] — the live built-in
//! cap bundles (builtin-role-freshness scope). These are what host callers SHOULD use: a new
//! built-in cap reaches already-seeded workspaces without a re-seed. The raw `lb_authz::resolve_caps`
//! / `resolve_subject_caps` (no builtins) stay available for tests that want the stored-row fold.
//!
//! Every host caller that resolves caps for a token/principal — the login mint (`role/gateway`),
//! the apikey auth/get path, the reminder fire re-resolve, the dashboard access_check — goes through
//! here so the fix is universal (one chokepoint, not five scattered `&LiveBuiltinRoleCaps` args).

use std::collections::BTreeSet;

use lb_authz::{resolve_caps_with, resolve_subject_caps_with, Subject};
use lb_store::{Store, StoreError};

use crate::authz::LiveBuiltinRoleCaps;

/// Resolve `user`'s effective caps in workspace `ws`, UNIONING the live built-in role bundles on top
/// of the stored records — the host's canonical resolve entry point. See [`resolve_caps_with`].
pub async fn resolve_caps_live(
    store: &Store,
    ws: &str,
    user: &str,
) -> Result<Vec<String>, StoreError> {
    resolve_caps_with(store, ws, user, &LiveBuiltinRoleCaps).await
}

/// [`resolve_subject_caps_with`] with [`LiveBuiltinRoleCaps`] baked in — the host's canonical entry
/// for a `key:`/`team:`/`role:` subject (no team-membership edge).
pub async fn resolve_subject_caps_live(
    store: &Store,
    ws: &str,
    subject: &Subject,
    caps: &mut BTreeSet<String>,
) -> Result<(), StoreError> {
    resolve_subject_caps_with(store, ws, subject, &LiveBuiltinRoleCaps, caps).await
}

/// The subject prefix whose grants go stale in a token. A `key:` subject already resolves its caps
/// live from the store on every request (`authenticate_apikey`), so it is never stale and needs
/// nothing here — the asymmetry is deliberate, not an omission.
const USER_PREFIX: &str = "user:";

/// Return a principal with freshly-resolved grants folded in, but **only** when `principal`'s cached
/// caps do not already satisfy `gate_tool` — otherwise `None`, meaning "use the one you have".
///
/// **The problem.** A session token is a cached projection of `resolve_caps`, taken once at login
/// (`role/gateway/src/session/mint_session.rs`). Grants written afterwards are invisible to it until
/// it expires — and one such write happens on a completely routine action:
/// [`grant_ui_scope_to_admin`](super::grant_ui_scope_to_admin) runs on every extension install and
/// grants the manifest's `[ui]`/`[[widget]]` scope to `role:workspace-admin`. So an extension
/// upgrade that adds a verb leaves every already-logged-in admin unable to call it for up to the
/// 12h token lifetime, with the page silently degrading and nothing in any log to explain it.
///
/// Observed live: `modbus` 0.1.7 → 0.1.8 added `device.status`/`point.status`; the install granted
/// both, a freshly minted token carried both, and an operator session minted minutes earlier was
/// refused both — every device on the page rendered `unknown`.
///
/// **The strategy: re-resolve only when the cached answer is NO.** The token's caps are consulted
/// first, exactly as before; the store is read *only* on the path that was about to return `Denied`.
/// The hot path (an allowed call) therefore costs nothing — no store read, no lock, no cache — and
/// the cost lands on a path that was already an error. That matters: [`resolve_caps_live`] is
/// O(teams) store reads, not something to pay on every dispatch.
///
/// **It can only ever agree with a re-login.** Caps are resolved server-side for the caller's own
/// `(sub, ws)`, so the widened principal is bounded by exactly what logging out and back in would
/// mint. Nothing here invents authority; it stops the cache from lying.
///
/// **It deliberately does not narrow.** A revoked grant rides its own mechanism (the `token_revoke`
/// tombstone in the gateway's `verify_token`, bounded by TTL). Folding revocation in here would also
/// silently drop the login-time viewer-floor and nav-reach caps, which live only in the token.
///
/// `gate_tool` must be the tool the gate will actually check (post-alias), so this asks the same
/// question the gate is about to ask. `None` is returned — leaving the caller's principal untouched —
/// whenever the repair does not apply: cached caps already allow it, the subject is not a `user:`,
/// the principal is delegated or run-scoped, the store read fails, the re-resolve adds nothing, or
/// the refreshed principal is *still* denied. A store error is deliberately not fatal: the caller
/// falls through to the ordinary gate and gets the ordinary `Denied`.
pub async fn refresh_grants_if_denied(
    store: &Store,
    principal: &lb_auth::Principal,
    ws: &str,
    gate_tool: &str,
) -> Option<lb_auth::Principal> {
    // 1. The cached answer, first and cheaply. An allowed call never reaches the store.
    if lb_mcp::authorize_tool(principal, ws, gate_tool).is_ok() {
        return None;
    }

    // 2. Only a user session goes stale this way. Bail before touching the store so a denied
    //    key/agent call costs exactly what it costs today.
    let bare_user = principal.sub().strip_prefix(USER_PREFIX)?;

    // 3. `with_live_grants` refuses these itself; checking here too skips a pointless store read on
    //    every denied agent call.
    if principal.constraint().is_some() || principal.run_id().is_some() {
        return None;
    }

    // 4. Re-resolve from the durable grant store. Grants are stored under the BARE user name — the
    //    resolver re-wraps it as `Subject::User` (the same handling `mint_session` documents).
    let resolved = resolve_caps_live(store, ws, bare_user).await.ok()?;

    // 5. Nothing new ⇒ the deny is genuine. Hand back `None` so the caller keeps its own principal
    //    and the gate produces the identical error it always did.
    if resolved.iter().all(|c| principal.caps().contains(c)) {
        return None;
    }

    let refreshed = principal.clone().with_live_grants(resolved);

    // 6. Adopt the refreshed principal ONLY if it changes the verdict. One that is still denied is
    //    discarded, so a genuine denial is never reported against a widened identity — the audit
    //    line names the caps the caller actually dispatched under.
    lb_mcp::authorize_tool(&refreshed, ws, gate_tool)
        .is_ok()
        .then_some(refreshed)
}
