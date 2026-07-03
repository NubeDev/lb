//! `nav.set_default(id)` — set the workspace-default nav (nav scope, resolved open question: an
//! explicit pointer, not "first visibility:workspace nav wins", for determinism). Writes the one
//! `workspace_nav_default:[ws]` pointer the resolver reads as its third tier. An empty `id` clears
//! it. Admin-ish write — gated by `mcp:nav.save:call` (the same authoring privilege that creates the
//! navs it points at; no separate cap for one pointer). The pointer is validated loosely: it may name
//! a nav that is later deleted/unshared, in which case the resolver falls through (nav scope, "Stale
//! pick" extended to the default tier).

use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_nav;
use super::error::NavError;
use super::store::write_default;

/// Set (or clear, on empty `id`) the workspace-default nav in `ws`, as `principal`, at time `now`.
/// Gated by `mcp:nav.save:call`. Idempotent (LWW on the single pointer).
pub async fn nav_set_default(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    now: u64,
) -> Result<(), NavError> {
    authorize_nav(principal, ws, "nav.save")?;
    write_default(store, ws, id, now).await?;
    Ok(())
}
