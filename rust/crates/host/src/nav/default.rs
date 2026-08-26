//! `nav.set_default(id)` / `nav.get_default()` — the ONE workspace-default nav pointer (nav scope,
//! resolved open question: an explicit pointer, not "first visibility:workspace nav wins", for
//! determinism). Both halves read/write the same `workspace_nav_default:[ws]` record the resolver
//! consults as its third tier. An empty `id` clears it.
//!
//! **The write is admin-ish, the read is member-level.** `set_default` is gated by
//! `mcp:nav.save:call` — the same authoring privilege that creates the navs it points at (no separate
//! cap for one pointer). `get_default` is gated by `mcp:nav.resolve:call`, because the pointer is
//! already the third tier of every member's own `nav.resolve`: naming which nav it points at tells a
//! caller nothing their own resolved menu doesn't, and gating the READ on the authoring cap would
//! make the pointer unreadable to exactly the people it shapes.
//!
//! **Why the read exists at all.** Without it the pointer was write-only: an admin could set the
//! default but nothing could ever show which nav *is* the default — the builder's "Default" badge
//! could only echo the last write in the current browser session, and vanished on reload
//! (rubix-ai#165). The no-lockout restore needs the same pointer to name the nav to put back
//! (rubix-ai#144). One route, two consumers.
//!
//! The pointer is validated loosely: it may name a nav that is later deleted/unshared, in which case
//! the resolver falls through (nav scope, "Stale pick" extended to the default tier) — and this read
//! reports it verbatim, a pointer, not a promise that the nav still resolves.

use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_nav;
use super::error::NavError;
use super::store::{read_default, write_default};

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

/// Read the workspace-default nav id in `ws`. `None` when none was ever set **or** it was cleared —
/// the same absence the resolver falls through on, so a caller cannot tell "never set" from
/// "cleared" (there is nothing to tell: both mean no default tier). Gated by `mcp:nav.resolve:call`
/// (member-level, see the module note). Workspace-walled by the id shape — a ws-B pointer is
/// structurally unreachable from ws A.
pub async fn nav_get_default(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Option<String>, NavError> {
    authorize_nav(principal, ws, "nav.resolve")?;
    Ok(read_default(store, ws).await?)
}
