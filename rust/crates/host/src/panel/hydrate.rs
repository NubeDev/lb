//! **The one hydration seam** (library-panels scope Decision: host-side, on `dashboard.get`). Expands
//! every ref cell (`panel_ref` set) into a full v3 `Cell` by resolving the `panel:{id}` record under
//! the **viewer's** three gates and copying its spec over the cell's layout — so the grid, the editor,
//! the read cache, and the standalone page all see plain v3 panels and need no ref-awareness beyond the
//! editor's link/unlink affordances. Headless MCP callers and export get resolved dashboards for free;
//! the placeholder/isolation/deny logic lives behind the capability wall exactly once.
//!
//! **The ref is authoritative** — the resolved spec overwrites whatever inline spec the cell carried
//! (a stale hydrated copy a client echoes back on save is ignored; `dashboard.save` strips it, see
//! `validate.rs`). A ref that cannot be resolved (deleted, unshared, or unreadable by this viewer)
//! degrades to an honest placeholder (`panel_missing = true`) — never a crash, never a leaked spec.
//! An inline cell (empty `panel_ref`) passes through untouched.

use lb_auth::Principal;
use lb_store::Store;

use super::get::panel_get;
use crate::dashboard::Cell;

/// Hydrate a dashboard's `cells` for `principal` in `ws`: resolve each ref cell's `panel_ref` to a full
/// v3 cell, leaving inline cells untouched. Errors resolving a ref never fail the read — they mark the
/// cell `panel_missing`.
pub async fn hydrate_cells(
    store: &Store,
    principal: &Principal,
    ws: &str,
    cells: Vec<Cell>,
) -> Vec<Cell> {
    let mut out = Vec::with_capacity(cells.len());
    for cell in cells {
        if cell.panel_ref.is_empty() {
            out.push(cell); // inline cell — unchanged
            continue;
        }
        out.push(hydrate_one(store, principal, ws, cell).await);
    }
    out
}

/// Resolve one ref cell. `panel_ref` is `panel:{id}` (or a bare slug); `panel_get` re-checks the three
/// gates under the viewer, so an unreadable panel yields the placeholder (deny is indistinguishable
/// from missing to the viewer — both are "not accessible", never the spec).
///
/// **Owner shortcut:** a viewer who OWNS the panel reads its own spec directly from the store. Owning a
/// panel implies being able to read it (you created it, possibly via `dashboard.pin` which writes the
/// panel as a side effect of attaching it to a dashboard — that caller has `mcp:dashboard.pin:call` but
/// not necessarily `mcp:panel.get:call`, and their just-pinned widget must render, not degrade to the
/// placeholder). Non-owner viewers still go through `panel_get`'s three gates (workspace → cap →
/// visibility).
async fn hydrate_one(store: &Store, principal: &Principal, ws: &str, cell: Cell) -> Cell {
    let id = cell.panel_ref.trim_start_matches("panel:");
    // Owner shortcut: a raw read first, then an owner check. Cheaper than `panel_get`'s full gate chain
    // when it hits, and the raw read returns `None` for a missing panel just like `panel_get` does.
    if let Ok(Some(p)) = super::store::read_panel(store, ws, id).await {
        if !p.deleted && p.owner == principal.owner_sub() {
            return resolved_cell(&cell, p);
        }
    }
    match panel_get(store, principal, ws, id).await {
        Ok(panel) => resolved_cell(&cell, panel),
        Err(_) => placeholder_cell(cell),
    }
}

/// Build the resolved cell: the panel's spec over the ref cell's **layout + marker + bounded
/// overrides** (title override + `panel_vars`). The `panel_ref` is KEPT so the editor can offer
/// link/unlink and a re-save round-trips the ref (not the hydrated spec).
fn resolved_cell(cell: &Cell, panel: super::model::Panel) -> Cell {
    let s = panel.spec;
    // Title override: a non-empty per-placement title wins over the panel's own title.
    let title = if cell.title.is_empty() {
        s.title.clone()
    } else {
        cell.title.clone()
    };
    Cell {
        // Layout + marker + per-placement overrides (from the ref cell).
        i: cell.i.clone(),
        x: cell.x,
        y: cell.y,
        w: cell.w,
        h: cell.h,
        panel_ref: cell.panel_ref.clone(),
        panel_vars: cell.panel_vars.clone(),
        panel_missing: false,
        title,
        // Spec (from the panel record) — authoritative.
        v: s.v,
        widget_type: s.widget_type,
        view: s.view,
        binding: s.binding,
        source: s.source,
        action: s.action,
        options: s.options,
        description: s.description,
        sources: s.sources,
        transformations: s.transformations,
        field_config: s.field_config,
        plugin_version: s.plugin_version,
    }
}

/// A dangling/unreadable ref → the honest placeholder: layout + marker kept, `panel_missing` set, and
/// no spec (the UI renders "panel not accessible"). Never leaks a stale spec.
fn placeholder_cell(cell: Cell) -> Cell {
    Cell {
        i: cell.i,
        x: cell.x,
        y: cell.y,
        w: cell.w,
        h: cell.h,
        panel_ref: cell.panel_ref,
        panel_vars: cell.panel_vars,
        panel_missing: true,
        title: cell.title,
        v: 0,
        widget_type: String::new(),
        view: String::new(),
        binding: serde_json::Value::Null,
        source: Default::default(),
        action: Default::default(),
        options: serde_json::Value::Null,
        description: String::new(),
        sources: Vec::new(),
        transformations: Vec::new(),
        field_config: serde_json::Value::Null,
        plugin_version: String::new(),
    }
}
