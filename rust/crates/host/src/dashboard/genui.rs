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

/// The genui component names this node can render — the same embedded-catalog set `check_genui_cell`
/// validates against, exposed so `dashboard.catalog` can tell an AI author which genui components exist
/// (widget-catalog scope). Sorted for a stable catalog response.
pub fn genui_component_names() -> Vec<String> {
    let mut names: Vec<String> = catalog().names.iter().cloned().collect();
    names.sort();
    names
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

/// Lenient-args normalization (the `typed_arg` precedent — serde strictness at an AI seam is a
/// five-turn tax): an LLM regularly JSON-encodes the IR as a STRING inside the genui block. If
/// `genui.ir` is a string that parses to a JSON object, replace it with the parsed object so the
/// validators — and the renderer — see the real IR. Anything else is left alone for the checker to
/// reject with its precise message. Returns `true` when it rewrote (the caller re-serializes).
pub fn normalize_genui_block(genui: &mut Value) -> bool {
    let Some(Value::String(raw)) = genui.get("ir") else {
        return false;
    };
    match serde_json::from_str::<Value>(raw) {
        Ok(parsed @ Value::Object(_)) => {
            genui["ir"] = parsed;
            true
        }
        _ => false,
    }
}

fn bad(cell_i: &str, msg: impl std::fmt::Display) -> DashboardError {
    DashboardError::BadInput(format!("cell {cell_i} (genui): {msg}"))
}

fn check_genui_cell(cell: &Cell) -> Result<(), DashboardError> {
    // The genui payload lives under `options.genui = { v, ir, meta? }`. An UN-AUTHORED draft — a genui
    // cell the author just added but hasn't generated an IR for yet — has no `genui` block (or one with
    // no `ir`). That is a legitimate savable draft (like a blank timeseries you configure later), NOT a
    // malformed spec: the view renders an "author me" placeholder, not a broken widget. We only validate
    // once an actual `ir` is present. A `genui` block with a NON-object `ir` is malformed and rejected.
    let Some(genui) = cell.options.get("genui") else {
        return Ok(());
    };
    if matches!(genui.get("ir"), None | Some(Value::Null)) {
        return Ok(()); // draft: block present but no IR authored yet.
    }
    check_genui_block(genui).map_err(|msg| bad(&cell.i, msg))
}

/// A compact, complete, valid IR appended to every structural rejection — the AI author writes the
/// dialect from foreign priors, so the error must SHOW the shape once instead of steering one field
/// per turn (live run 2026-07-06 round 2 burned 5 turns fixing one defect at a time).
const IR_TEMPLATE: &str = r#"{"v":1,"surface":{"surfaceId":"s1","root":"root"},"components":{"root":{"id":"root","component":"stack","children":["t1"]},"t1":{"id":"t1","component":"text","props":{"value":"hi"}}}}"#;

/// Structurally validate a `{ v, ir, meta? }` genui block whose `ir` is present. Shared by the
/// dashboard save path ([`check_genui_cells`], which skips IR-less drafts first) and the channel
/// `rich_result` post path (`channel::genui_check`, where a missing IR is itself an error — a posted
/// preview with nothing to render is a broken post, not a draft).
///
/// Error messages are written FOR the agent loop: every independent defect is collected into ONE
/// message (a first-failure return teaches one field per turn — a five-turn tax, measured live), each
/// names the fix, and the canonical template is appended so a from-scratch rewrite lands in one retry.
pub fn check_genui_block(genui: &Value) -> Result<(), String> {
    let cat = catalog();

    // Shape gates — without an IR object there is nothing further to enumerate.
    let ir = match genui.get("ir") {
        None | Some(Value::Null) => {
            return Err(with_template(
                "options.genui has no `ir` object".to_string(),
            ));
        }
        Some(ir) if !ir.is_object() => {
            return Err(with_template(
                "options.genui.ir must be a JSON object, not a string — pass the IR inline, unquoted"
                    .to_string(),
            ));
        }
        Some(ir) => ir,
    };

    // Size bound — measure the WHOLE block (IR + meta), as it will be persisted. Fatal alone: the
    // right fix is a smaller widget, not field edits.
    let size = serde_json::to_vec(genui)
        .map(|v| v.len())
        .unwrap_or(usize::MAX);
    if size > GENUI_MAX_BYTES {
        return Err(format!(
            "spec is too large ({size} bytes > {GENUI_MAX_BYTES}); simplify the widget"
        ));
    }

    let mut defects: Vec<String> = Vec::new();

    // IR schema version present and known (not newer than this node's catalog can render).
    match ir.get("v").and_then(Value::as_u64) {
        None => defects.push("IR has no numeric `v` (the IR requires `\"v\": 1`)".to_string()),
        Some(v) if v == 0 || v > cat.version => defects.push(format!(
            "IR schema v{v} is unknown to this node (catalog v{})",
            cat.version
        )),
        Some(_) => {}
    }

    // Every component resolves in the catalog, names itself via `component` (NOT `type` — the most
    // common LLM dialect slip), and repeats its map key as `id` (the TS validator's `id-mismatch`).
    let components = match ir.get("components").and_then(Value::as_object) {
        None => {
            defects.push("IR has no `components` map".to_string());
            return Err(join_defects(defects));
        }
        Some(c) if c.is_empty() => {
            defects.push("IR has no components".to_string());
            return Err(join_defects(defects));
        }
        Some(c) => c,
    };
    for (id, comp) in components {
        match comp.get("component").and_then(Value::as_str) {
            Some(name) if !cat.names.contains(name) => defects.push(format!(
                "component \"{name}\" (id {id}) is not in the catalog"
            )),
            Some(_) => {}
            None if comp.get("type").is_some() => defects.push(format!(
                "component {id} uses `type`; the IR field is `component` (e.g. {{\"id\":\"{id}\",\"component\":\"stack\",...}})"
            )),
            None => defects.push(format!("component {id} has no `component` name")),
        }
        if comp.get("id").and_then(Value::as_str) != Some(id.as_str()) {
            defects.push(format!(
                "component {id} must repeat its map key as `id: \"{id}\"`"
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
        defects.push(
            "IR needs `surface: {\"surfaceId\": \"...\", \"root\": \"<a defined component id>\"}`"
                .to_string(),
        );
    }

    if defects.is_empty() {
        Ok(())
    } else {
        Err(join_defects(defects))
    }
}

fn join_defects(defects: Vec<String>) -> String {
    with_template(if defects.len() == 1 {
        defects.into_iter().next().expect("non-empty")
    } else {
        format!(
            "{} defects — fix ALL in one retry: {}",
            defects.len(),
            defects.join("; ")
        )
    })
}

fn with_template(msg: String) -> String {
    format!("{msg}. A minimal valid IR: {IR_TEMPLATE}")
}
