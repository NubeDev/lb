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
use super::model::{NavPref, MAX_PINNED};
use super::store::{read_pref, write_pref};

/// Read the caller's own active pick. Absent → an empty [`NavPref`] (no pick). Member-level.
pub async fn nav_pref_get(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<NavPref, NavError> {
    // Gated by the resolve cap — reading one's own pick is a read of one's own menu state.
    authorize_nav(principal, ws, "nav.resolve")?;
    Ok(read_pref(store, ws, principal.sub())
        .await?
        .unwrap_or_default())
}

/// Set the caller's OWN active pick and/or their **pinned favorites** (hide-and-pins scope). Both
/// fields are partial-write: `nav_id: None` leaves the pick untouched (so a pin toggle never
/// clobbers it), `Some("")` clears it, `Some(id)` sets it; `pinned: None` leaves the pins untouched
/// (the pre-pins callers keep their exact behavior), `Some(refs)` replaces them. Pins are bounded by
/// [`MAX_PINNED`] (`BadInput` over — never truncated); refs are opaque strings in the shared grammar
/// (bare surface key | `ext:<id>` | `dashboard:<id>`). Always keyed by `principal.sub()` — a caller
/// cannot set another user's pick or pins (member-owned). Member-level. LWW at logical time `now`.
pub async fn nav_pref_set(
    store: &Store,
    principal: &Principal,
    ws: &str,
    nav_id: Option<&str>,
    pinned: Option<Vec<String>>,
    now: u64,
) -> Result<NavPref, NavError> {
    authorize_nav(principal, ws, "nav.resolve")?;
    let existing = read_pref(store, ws, principal.sub())
        .await?
        .unwrap_or_default();
    let pinned = match pinned {
        Some(p) => {
            if p.len() > MAX_PINNED {
                return Err(NavError::BadInput(format!(
                    "{} pins exceeds cap {MAX_PINNED}",
                    p.len()
                )));
            }
            if p.iter().any(|r| r.trim().is_empty()) {
                return Err(NavError::BadInput("pin ref must be non-empty".into()));
            }
            p
        }
        None => existing.pinned,
    };
    let pref = NavPref {
        active: nav_id.map(str::to_string).unwrap_or(existing.active),
        pinned,
        updated_ts: now,
    };
    write_pref(store, ws, principal.sub(), &pref).await?;
    Ok(pref)
}
