//! `dashboard.save(id, title, cells)` — one idempotent UPSERT for create+update (dashboard scope,
//! "MCP surface"; a fresh id creates, an existing id updates — not two verbs). Synchronous (one small
//! layout record; not a job). Gated by `mcp:dashboard.save:call`.
//!
//! **Ownership on update:** a save against an existing dashboard is allowed only for its owner — a
//! non-owner with the save cap still cannot overwrite someone else's dashboard (mirrors `share_doc`'s
//! owner check). Create stamps `owner = principal`; `visibility` is set via `dashboard.share`, so
//! save **preserves** the existing visibility (it never silently re-privatizes a shared dashboard).

use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_dashboard;
use super::error::DashboardError;
use super::model::{Cell, Dashboard, Variable, Visibility};
use super::store::{read_dashboard, write_dashboard};

/// Upsert dashboard `id` in `ws` with `title` + `cells`, as `principal`, at logical time `now`.
/// Creates on a fresh id (owner = principal, visibility = private); updates an existing one
/// (owner-only). Returns the persisted record.
pub async fn dashboard_save(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    title: &str,
    cells: Vec<Cell>,
    variables: Vec<Variable>,
    now: u64,
) -> Result<Dashboard, DashboardError> {
    authorize_dashboard(principal, ws, "dashboard.save")?;
    if id.is_empty() {
        return Err(DashboardError::BadInput("empty dashboard id".into()));
    }

    // Preserve owner + visibility across an update; only the owner may update. A tombstoned record
    // is treated as absent — a save with that id resurrects it under the new owner (create).
    let (owner, visibility) = match read_dashboard(store, ws, id).await?.filter(|d| !d.deleted) {
        Some(existing) => {
            if existing.owner != principal.sub() {
                return Err(DashboardError::Denied);
            }
            (existing.owner, existing.visibility)
        }
        None => (principal.sub().to_string(), Visibility::Private),
    };

    let dashboard = Dashboard {
        id: id.to_string(),
        title: title.to_string(),
        owner,
        visibility,
        cells,
        variables,
        updated_ts: now,
        deleted: false,
    };
    write_dashboard(store, ws, &dashboard).await?;
    Ok(dashboard)
}
