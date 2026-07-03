//! `nav.pref.get` / `nav.pref.set` — the per-user active pick (`nav_pref:[ws, user]`; nav scope, "A
//! per-user active pick"). A member always curates their OWN pick — the pick is keyed by the
//! authenticated principal's `sub`, never a body field, so a member can never set another user's pick
//! (the member-owned test). Member-level: gated by `mcp:nav.resolve:call` (the same read privilege
//! that resolves their menu — curating which nav you use is part of resolving your own menu), so it
//! needs no admin cap. A pick pointing at a deleted/unshared nav is harmless — the resolver falls
//! through to the next tier (nav scope, "Stale pick").

use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_nav;
use super::error::NavError;
use super::model::NavPref;
use super::store::{read_pref, write_pref};

/// Read the caller's own active pick. Absent → an empty [`NavPref`] (no pick). Member-level.
pub async fn nav_pref_get(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<NavPref, NavError> {
    // Gated by the resolve cap — reading one's own pick is a read of one's own menu state.
    authorize_nav(principal, ws, "nav.resolve")?;
    Ok(read_pref(store, ws, principal.sub()).await?.unwrap_or_default())
}

/// Set the caller's OWN active pick to `nav_id` (empty clears it), at logical time `now`. Always keyed
/// by `principal.sub()` — a caller cannot set another user's pick (member-owned). Member-level.
pub async fn nav_pref_set(
    store: &Store,
    principal: &Principal,
    ws: &str,
    nav_id: &str,
    now: u64,
) -> Result<NavPref, NavError> {
    authorize_nav(principal, ws, "nav.resolve")?;
    let pref = NavPref {
        active: nav_id.to_string(),
        updated_ts: now,
    };
    write_pref(store, ws, principal.sub(), &pref).await?;
    Ok(pref)
}
