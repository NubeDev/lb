//! The store-admin gates. The outer `mcp:store.status|compact:call` gate runs at the
//! dispatcher; here the **store-surface** capabilities run through the shared `caps::check`
//! chokepoint (workspace-first §3.6, then capability §3.5) — the same two-gate posture as
//! `store.write`'s per-table gate. Denials are opaque ([`StoreAdminError::Denied`]).
//!
//! Grammar (online-compaction scope): reading operational status is `store:status:read`
//! (covered by the viewer-tier `store:*:read` wildcard — byte counts, no records); triggering
//! a pass is `store:compact:run` — a distinct `run` action precisely so the broad author
//! `store:*:write` wildcard can never imply "pause every writer on the node".

use lb_auth::Principal;
use lb_caps::{check, Action, Decision, Request, Surface};

use super::error::StoreAdminError;

/// Authorize reading the store's operational status in `ws` (`store:status:read`).
pub fn authorize_store_status(principal: &Principal, ws: &str) -> Result<(), StoreAdminError> {
    let req = Request::new(ws, Surface::Store, "status", Action::Read);
    match check(principal, &req) {
        Decision::Allowed => Ok(()),
        Decision::Denied(_) => Err(StoreAdminError::Denied),
    }
}

/// Authorize triggering a compaction pass from `ws` (`store:compact:run`).
pub fn authorize_store_compact(principal: &Principal, ws: &str) -> Result<(), StoreAdminError> {
    let req = Request::new(ws, Surface::Store, "compact", Action::Run);
    match check(principal, &req) {
        Decision::Allowed => Ok(()),
        Decision::Denied(_) => Err(StoreAdminError::Denied),
    }
}
