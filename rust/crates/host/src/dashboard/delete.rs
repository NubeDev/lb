//! `dashboard.delete(id)` — tombstone-upsert (dashboard scope, "MCP surface"; §6.8 idempotent). A
//! re-delete is a no-op, and a delete of an absent dashboard is a no-op (not an error) — the
//! idempotency the sync path relies on. Gated by `mcp:dashboard.delete:call`. Owner-only UNLESS the
//! caller also holds `mcp:dashboard.delete_any:call` (an admin-granted cap, checked second so a
//! non-admin never pays its cost) — an admin needs to clean up dashboards other members own (e.g. an
//! ex-member's clutter); this is the one owner-only gate in the dashboard surface with an admin
//! override (`save`/`share` stay strictly owner-only, matching panel/nav/assets §3.5).

use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_dashboard;
use super::error::DashboardError;
use super::store::{read_dashboard, write_dashboard};

/// Soft-delete dashboard `id` in `ws` as `principal`, at logical time `now`. Idempotent: an absent
/// or already-tombstoned dashboard is a no-op. The owner may always delete; a non-owner may delete
/// only if also granted `mcp:dashboard.delete_any:call` (admin override).
pub async fn dashboard_delete(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    now: u64,
) -> Result<(), DashboardError> {
    authorize_dashboard(principal, ws, "dashboard.delete")?;

    match read_dashboard(store, ws, id).await? {
        // Already gone (absent or tombstoned) — idempotent no-op.
        None => Ok(()),
        Some(d) if d.deleted => Ok(()),
        Some(mut d) => {
            if d.owner != principal.owner_sub()
                && authorize_dashboard(principal, ws, "dashboard.delete_any").is_err()
            {
                return Err(DashboardError::Denied);
            }
            d.deleted = true;
            d.updated_ts = now;
            write_dashboard(store, ws, &d).await?;
            Ok(())
        }
    }
}
