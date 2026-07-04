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
    // v3 record bounds — reject an over-cap fieldConfig/transform list rather than store it unbounded
    // (panel-model scope: keep the dashboard record small for the roster/list read). The host is the
    // boundary; the editor mirrors the caps for a friendly error.
    super::bounds::check_cells_bounds(&cells)?;
    // `view:"genui"` cells carry a typed IR in `options.genui`; validate it structurally at write time
    // (genui-scope Decision 6) so a malformed genui cell is rejected loudly here, not degraded at view
    // time. Same authority for every writer — shell, `POST /mcp/call`, routed Zenoh, external-agent.
    super::genui::check_genui_cells(&cells)?;
    // Every cell's `view` NAME is validated against the embedded widget catalog (widget-catalog scope,
    // Slice A): a hallucinated view (an unknown built-in, a malformed `ext:` key) is rejected loudly
    // HERE — same authority for every writer — so a broken tile never persists (the reported G4 bug).
    // Structural only (view name, not option keys); `ext:<id>/<widget>` is checked well-formed, not
    // resolved against installs (so the save stays store-only). `genui`'s IR is validated above.
    super::views::check_view_cells(&cells)?;

    // Library-panel refs (library-panels scope: "validate at write, tolerate at read"). Every ref
    // cell's `panel_ref` must resolve in-workspace under the saver NOW (loud `BadInput` otherwise); the
    // ref is authoritative, so any echoed hydrated spec is stripped — a ref cell is stored with only
    // layout + the ref + bounded overrides. Inline cells pass through untouched.
    let cells = crate::panel::validate_and_strip_refs(store, principal, ws, cells)
        .await
        .map_err(DashboardError::BadInput)?;

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
        // Pin our panel-model document version at save (viz panel-model scope). v3 is the current
        // shape; an older saved doc keeps its lower value until the migration path reads it.
        schema_version: super::model::SCHEMA_VERSION,
        updated_ts: now,
        deleted: false,
    };
    write_dashboard(store, ws, &dashboard).await?;

    // Return a HYDRATED record, mirroring `dashboard.get`. We just stripped ref cells to layout+ref
    // before write (the ref is authoritative; the spec lives on the panel record), so the in-memory
    // `dashboard.cells` are the stripped form — empty `view`/`widget_type`/`sources`. A client that
    // `setCurrent`s the save's return (every dashboard edit: drag/resize/add/remove/duplicate) would
    // otherwise render every ref cell as "Unsupported widget" until the next reload. Re-hydrating the
    // returned value (not the persisted record) closes that gap; the on-disk record stays stripped.
    let mut dashboard = dashboard;
    dashboard.cells =
        crate::panel::hydrate_cells(store, principal, ws, std::mem::take(&mut dashboard.cells))
            .await;
    Ok(dashboard)
}
