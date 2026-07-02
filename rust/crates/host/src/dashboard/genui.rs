//! `view:"genui"` cell validation on save (genui-scope Decision 6 — the ONE host-side change in the
//! slice; no new verb, cap, or table). A genui cell persists `options.genui = { v, ir, meta? }` where
//! `ir` is the typed, versioned GenUI IR the renderer consumes. The host is the boundary: it structurally
//! validates every genui cell at write time so a malformed one is REJECTED loudly, not degraded at view
//! time. This closes the headless-MCP-author gap — a `POST /mcp/call` / routed-Zenoh / external-agent
//! writer gets the exact same rejection the shell gives (the shell also validates at accept; this is the
//! server-side authority the shell mirrors).
//!
//! Three checks, mirroring the accept step in `@nube/genui`'s `authoring.acceptIr`:
//!   1. IR schema `v` present and KNOWN (`≤` the catalog's version — a future `v` can't be rendered).
//!   2. the whole `options.genui` block within the ~8 KB bound (an oversized catalog spec is almost
//!      certainly a bad generation; there is no second persistence path).
//!   3. every `component` name in the IR resolves in the generated catalog JSON (the embedded artifact,
//!      not the TS) — the structural "only the catalog may be instantiated" constraint, server-side.
//!
//! The catalog JSON is GENERATED from `defineCatalog` by `pnpm --filter @nube/genui gen:skill` and
//! checked in beside this file; CI fails on a dirty diff so the node never validates against a stale
//! catalog (genui-scope "The codegen chain is named").

use std::collections::HashSet;
use std::sync::OnceLock;

use serde_json::Value;

use super::error::DashboardError;
use super::model::Cell;

/// The generated catalog JSON (name-set + schema version) the host validates against. `include_str!`
/// of the checked-in artifact — the same "embed a generated asset" pattern as `prefs/builtin/*.mf`.
const CATALOG_JSON: &str = include_str!("genui_catalog.json");

/// Max serialized size of the whole `options.genui` block. Mirrors `@nube/genui`'s `GENUI_MAX_BYTES`.
pub const GENUI_MAX_BYTES: usize = 8 * 1024;

struct Catalog {
    /// IR schema version this catalog targets — a cell's `v` must be `<=` this to be renderable.
    version: u64,
    names: HashSet<String>,
}

fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let doc: Value =
            serde_json::from_str(CATALOG_JSON).expect("genui_catalog.json is valid JSON");
        let version = doc.get("v").and_then(Value::as_u64).unwrap_or(0);
        let names = doc
            .get("components")
            .and_then(Value::as_array)
            .map(|comps| {
                comps
                    .iter()
                    .filter_map(|c| c.get("name").and_then(Value::as_str))
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        Catalog { version, names }
    })
}

/// Validate every `view:"genui"` cell in a save. Non-genui cells are ignored. Returns the first
/// failure loudly (the same `BadInput` shape `bounds.rs` uses → `ToolError::BadInput` over MCP/HTTP).
pub fn check_genui_cells(cells: &[Cell]) -> Result<(), DashboardError> {
    for cell in cells {
        if cell.view == "genui" {
            check_genui_cell(cell)?;
        }
    }
    Ok(())
}

fn bad(cell_i: &str, msg: impl std::fmt::Display) -> DashboardError {
    DashboardError::BadInput(format!("cell {cell_i} (genui): {msg}"))
}

fn check_genui_cell(cell: &Cell) -> Result<(), DashboardError> {
    let cat = catalog();

    // The genui payload lives under `options.genui = { v, ir, meta? }`.
    let genui = cell
        .options
        .get("genui")
        .ok_or_else(|| bad(&cell.i, "view is \"genui\" but options.genui is missing"))?;

    // Size bound — measure the WHOLE block (IR + meta), as it will be persisted.
    let size = serde_json::to_vec(genui)
        .map(|v| v.len())
        .unwrap_or(usize::MAX);
    if size > GENUI_MAX_BYTES {
        return Err(bad(
            &cell.i,
            format!("spec is too large ({size} bytes > {GENUI_MAX_BYTES}); simplify the widget"),
        ));
    }

    let ir = genui
        .get("ir")
        .ok_or_else(|| bad(&cell.i, "options.genui.ir is missing"))?;

    // IR schema version present and known (not newer than this node's catalog can render).
    let v = ir
        .get("v")
        .and_then(Value::as_u64)
        .ok_or_else(|| bad(&cell.i, "IR has no numeric `v`"))?;
    if v == 0 || v > cat.version {
        return Err(bad(
            &cell.i,
            format!(
                "IR schema v{v} is unknown to this node (catalog v{})",
                cat.version
            ),
        ));
    }

    // Every component name resolves in the catalog.
    let components = ir
        .get("components")
        .and_then(Value::as_object)
        .ok_or_else(|| bad(&cell.i, "IR has no `components` map"))?;
    if components.is_empty() {
        return Err(bad(&cell.i, "IR has no components"));
    }
    for (id, comp) in components {
        let name = comp
            .get("component")
            .and_then(Value::as_str)
            .ok_or_else(|| bad(&cell.i, format!("component {id} has no `component` name")))?;
        if !cat.names.contains(name) {
            return Err(bad(
                &cell.i,
                format!("component \"{name}\" (id {id}) is not in the catalog"),
            ));
        }
    }

    // Root must name a defined component (a headless writer can't ship a dangling root).
    let root = ir
        .get("surface")
        .and_then(|s| s.get("root"))
        .and_then(Value::as_str)
        .unwrap_or("");
    if root.is_empty() || !components.contains_key(root) {
        return Err(bad(
            &cell.i,
            "surface root is empty or not a defined component",
        ));
    }
    Ok(())
}
