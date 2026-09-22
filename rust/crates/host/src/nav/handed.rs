//! The menus a principal was **handed** (entity-scoped-data scope): every team-shared nav they can
//! read, plus the workspace default. This is the ACCESS view of the nav tiers, distinct from
//! `pick_nav`, which picks the ONE menu to draw:
//! - a personal pick (tier 1) or the built-in fallback never appears here — a choice a member makes
//!   for themself ("show all pages", picking one of their teams' menus) can neither widen nor zero
//!   what they may read;
//! - EVERY readable team menu counts, not just the first by id — a member of two groups reaches the
//!   union of both, whichever menu the rail happens to draw;
//! - the admin no-lockout rule does not apply: admins are unrestricted before this is consulted.

use lb_auth::Principal;
use lb_store::Store;

use super::error::NavError;
use super::model::{Nav, Visibility};
use super::resolve::readable_nav;
use super::store::{read_default, scan_navs};
use super::visibility::may_read_nav;

pub(crate) async fn handed_navs(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Vec<Nav>, NavError> {
    let mut out = Vec::new();
    for nav in scan_navs(store, ws).await? {
        if nav.deleted || nav.visibility != Visibility::Team {
            continue;
        }
        if may_read_nav(store, principal, ws, &nav).await.is_ok() {
            out.push(nav);
        }
    }
    if let Some(default_id) = read_default(store, ws).await? {
        if let Some(nav) = readable_nav(store, principal, ws, &default_id).await? {
            if !out.iter().any(|n| n.id == nav.id) {
                out.push(nav);
            }
        }
    }
    Ok(out)
}
