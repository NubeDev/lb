//! `dashboard_head(id)` — the same four gates as [`dashboard_get`](super::get::dashboard_get), and
//! the board's HEADING ONLY. For a caller that needs to know "may this principal read it, and what is
//! it called?" and nothing else.
//!
//! That caller is the nav resolver, and the difference is not cosmetic. `dashboard_get` hydrates the
//! page: every cell that references a library panel is resolved through `panel_get` under the
//! viewer's own gates, which is a store read per cell. A menu item then keeps the title and drops all
//! of it. Measured on a real workspace (rubix-ai `esr`, a 48-item menu): the resolve read ~2.4 MB of
//! dashboard and panel records to produce an 11.8 KB response, and one 79 KB board was read twenty
//! times because twenty menu items pointed at it. Nav items are the only reader that fans out like
//! this, so the cheap read lives here and the full read stays exactly as it was.
//!
//! The GATES are identical and run in the identical order — workspace + capability before any fetch,
//! then the record, then membership/visibility, then record reach. This is load-bearing: a cheaper
//! read that skipped a gate would be a way to ask "does this board exist?" without the cap for it.
//! The only thing dropped is hydration, which is presentation and cannot widen access.
//!
//! One responsibility: the gated heading of a dashboard.

use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_dashboard;
use super::error::DashboardError;
use super::reach_gate::reach_allows;
use super::store::read_dashboard;
use super::visibility::may_read_dashboard;

/// What a menu row needs from a board it points at: its title, and the fact that reading it was
/// allowed at all. Deliberately one field — add another only when a caller genuinely renders it,
/// because every field is one more thing the cheap read has to keep true.
#[derive(Debug, Clone)]
pub struct DashboardHead {
    /// The board's own title — a nav item with no authored label of its own falls back to it.
    pub title: String,
}

/// Read dashboard `id`'s heading in `ws` for `principal`, if all four gates pass. Same errors as
/// [`dashboard_get`](super::get::dashboard_get): a tombstoned or absent board is `NotFound`, a board
/// outside the caller's membership or record reach is `Denied`.
pub async fn dashboard_head(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<DashboardHead, DashboardError> {
    // Gates 1 + 2: workspace isolation, then the read capability — before any fetch.
    authorize_dashboard(principal, ws, "dashboard.get")?;

    let dashboard = read_dashboard(store, ws, id)
        .await?
        .filter(|d| !d.deleted)
        .ok_or(DashboardError::NotFound)?;

    // Gate 3: membership/visibility.
    may_read_dashboard(store, principal, ws, &dashboard).await?;

    // Gate 4: record reach — unarmed unless the caller's token carries record-granular reach.
    if !reach_allows(principal, ws, &dashboard) {
        return Err(DashboardError::Denied);
    }

    Ok(DashboardHead {
        title: dashboard.title,
    })
}
