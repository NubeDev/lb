//! `dashboard.save` ref handling (library-panels scope Decision: "validate at write, tolerate at
//! read"). Two jobs, one pass over the cells:
//!
//!  1. **Validate** every `panel_ref` resolves **in this workspace** under the saver — a save naming a
//!     ref that doesn't resolve is rejected **loudly** (`BadInput`, naming the ref) at author time, so a
//!     typo/cross-ws ref never persists silently. (Later dangling — the panel force-deleted *after* the
//!     save — is tolerated: it degrades to the placeholder at hydration, `hydrate.rs`.)
//!  2. **Strip the echoed spec** — the ref is authoritative, so a ref cell is stored with **only**
//!     layout + the ref + the bounded overrides (`title`, `panel_vars`); any hydrated spec a client
//!     echoed back is dropped, preventing accidental de-linking (a client can't overwrite the panel by
//!     re-sending a stale copy).
//!
//! Returns the cells to persist (inline cells untouched, ref cells stripped). An empty `panel_ref`
//! (inline cell) is never touched.

use lb_auth::Principal;
use lb_store::Store;

use super::get::panel_get;
use crate::dashboard::Cell;

/// Validate + normalize a dashboard's cells before write. Returns `Err(message)` naming the first ref
/// that does not resolve in-workspace; else the cells to store (ref cells stripped to layout+ref+
/// overrides). The caller (`dashboard.save`) wraps the error in `DashboardError::BadInput`.
///
/// **Owner shortcut:** a saver who OWNS the panel a cell references reads it through the raw store
/// (mirrors `hydrate_one`). This matters for `dashboard.pin`, whose caller has `mcp:dashboard.pin:call`
/// but not necessarily `mcp:panel.get:call`, AND for a dashboard owner re-saving a dashboard whose
/// prior pin-created refs are theirs — both must round-trip without forcing a separate panel.read cap.
pub async fn validate_and_strip_refs(
    store: &Store,
    principal: &Principal,
    ws: &str,
    cells: Vec<Cell>,
) -> Result<Vec<Cell>, String> {
    let mut out = Vec::with_capacity(cells.len());
    for cell in cells {
        if cell.panel_ref.is_empty() {
            out.push(cell); // inline cell — stored verbatim
            continue;
        }
        let id = cell.panel_ref.trim_start_matches("panel:");
        // Owner shortcut: a raw read first; if it hits AND the saver owns the panel, treat as resolved.
        let owner_owned = match super::store::read_panel(store, ws, id).await {
            Ok(Some(p)) if !p.deleted && p.owner == principal.owner_sub() => true,
            _ => false,
        };
        if !owner_owned {
            // Non-owner (or missing): the ref must resolve for the saver right now (in-workspace,
            // readable, shared) — loud on failure.
            panel_get(store, principal, ws, id).await.map_err(|_| {
                format!(
                    "panel_ref does not resolve in workspace: {}",
                    cell.panel_ref
                )
            })?;
        }
        out.push(stripped_ref(cell));
    }
    Ok(out)
}

/// A ref cell stored with only layout + the ref + the bounded overrides — the echoed spec dropped.
fn stripped_ref(cell: Cell) -> Cell {
    Cell {
        i: cell.i,
        x: cell.x,
        y: cell.y,
        w: cell.w,
        h: cell.h,
        panel_ref: cell.panel_ref,
        panel_vars: cell.panel_vars,
        panel_missing: false,
        // The one spec-ish field that IS a per-placement override: the title. Everything else cleared.
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
        // P1 spec-ish fields (queryOptions/transparent/links) are spec, not per-placement
        // overrides — cleared with the rest of the echoed spec.
        ..Cell::default()
    }
}
