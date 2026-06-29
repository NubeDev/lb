//! The gated prefs host verbs — the capability chokepoint over the raw `lb_prefs` store layer.
//! Each authorizes first, then calls the pure crate. "OWN" verbs force the target `user` to the
//! caller's `sub` so a holder of `prefs.get`/`prefs.set` can only ever touch their own record
//! (structural, beyond the cap) — the prefs-scope deny requirement.

use lb_auth::Principal;
use lb_prefs::{
    get_user_prefs, resolve_chain, set_user_prefs, set_workspace_prefs, Prefs, ResolvedPrefs,
};
use lb_store::Store;

use super::authorize::authorize_prefs;
use super::error::PrefsSvcError;

/// `prefs.get` (read OWN) — the caller's own stored, nullable prefs (`None` if unset). The `user`
/// is ALWAYS the caller's `sub`; there is no parameter to read another user's record.
pub async fn prefs_get(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Option<Prefs>, PrefsSvcError> {
    authorize_prefs(principal, ws, "prefs.get")?;
    Ok(get_user_prefs(store, ws, principal.sub()).await?)
}

/// `prefs.set` (write OWN) — merge `patch` into the caller's own record. Forced to `principal.sub()`;
/// a caller cannot write a different user's record.
pub async fn prefs_set(
    store: &Store,
    principal: &Principal,
    ws: &str,
    patch: &Prefs,
) -> Result<(), PrefsSvcError> {
    authorize_prefs(principal, ws, "prefs.set")?;
    set_user_prefs(store, ws, principal.sub(), patch).await?;
    Ok(())
}

/// `prefs.resolve` (read OWN) — fold the chain for the caller, with an optional self-scoped request
/// override (e.g. "preview in es") that never writes the record.
pub async fn prefs_resolve(
    store: &Store,
    principal: &Principal,
    ws: &str,
    override_: Option<Prefs>,
) -> Result<ResolvedPrefs, PrefsSvcError> {
    authorize_prefs(principal, ws, "prefs.resolve")?;
    Ok(resolve_chain(store, ws, principal.sub(), override_).await?)
}

/// `prefs.set_default` (ADMIN) — set the workspace-default prefs. Gated by the admin-only
/// `mcp:prefs.set_default:call`; a non-admin (lacking that cap) is denied opaquely.
pub async fn prefs_set_default(
    store: &Store,
    principal: &Principal,
    ws: &str,
    patch: &Prefs,
) -> Result<(), PrefsSvcError> {
    authorize_prefs(principal, ws, "prefs.set_default")?;
    set_workspace_prefs(store, ws, patch).await?;
    Ok(())
}
