//! `dashboard.catalog` — the widget palette as an MCP read verb (widget-catalog scope, Slice A). One
//! read returns the whole authoring vocabulary an AI (or the RN app, or the web shell) needs to compose
//! a dashboard page: every built-in `view` with its per-view config-field schema, the workspace's
//! installed extension `[[widget]]` tiles, and the genui component names. Modeled on
//! [`tools.catalog`](../../tools/catalog.rs) — "the menu IS the palette."
//!
//! Three sources merged into one `{ v, views, extWidgets, genuiComponents }` document:
//!   1. **built-ins** — `include_str!`'d from `widget_catalog.json`, the host-owned source of truth the
//!      save-validator ([`super::views`]) keys off the SAME file (verb + validator can't disagree).
//!   2. **ext tiles** — folded generically from `ext.list` (widget-catalog scope, rule 10): each
//!      installed extension's `[[widget]]` tiles as opaque `{ext, widget, label, icon, data, scope}`.
//!      The id is DATA, never branched on. Workspace-scoped — a ws-B caller sees only ws-B's tiles.
//!   3. **genui** — the component NAMES from the genui catalog ([`super::genui::genui_component_names`]).
//!
//! Needs the full `&Arc<Node>` (like `nav.resolve`) for the `ext.list` discovery, so it is dispatched
//! via its own branch BEFORE the generic store-only `dashboard.` branch. Gated by
//! `mcp:dashboard.catalog:call` (member-level, workspace-first) — reading the palette grants nothing
//! but knowledge; the write stays gated on `dashboard.save`. Self-describes via a `ToolDescriptor` so
//! it appears in `tools.catalog`.

use std::sync::Arc;

use lb_auth::Principal;
use lb_mcp::{authorize_tool, ToolDescriptor, ToolError};
use serde::Serialize;
use serde_json::Value;

use crate::boot::Node;
use crate::ext::ext_list;

/// The embedded built-in palette — the same host-owned file `super::views` validates saves against.
const CATALOG_JSON: &str = include_str!("widget_catalog.json");

/// One installed extension `[[widget]]` tile, as the catalog surfaces it — opaque `{ext, widget}`
/// coordinates plus its declared presentation/scope (widget-catalog scope). The `widget` key is the
/// tile's federation id: an author composes `view:"ext:<ext>/<widget>"`. No config schema (the
/// extension owns its config — same v1 limit as the human palette).
#[derive(Debug, Clone, Serialize)]
pub struct ExtWidget {
    /// The opaque extension id (never branched on — rule 10).
    pub ext: String,
    /// The tile's federation id within the extension (its `[[widget]]` label, slugged by the author
    /// into the `ext:<ext>/<widget>` view key). We surface the human `label` separately.
    pub widget: String,
    /// The human label the extension declared for the tile.
    pub label: String,
    /// A lucide icon name (empty = default).
    pub icon: String,
    /// `true` = a frames-in data tile (carries `sources[]`, receives shell-resolved frames); `false` =
    /// a self-fetching tile (ext-widget-source-binding scope).
    pub data: bool,
    /// The read-only MCP tool scope the tile may call through the host bridge (already ∩ install-grant).
    pub scope: Vec<String>,
    /// The tile's declarative panel options (ext-widget-panel-options scope) — relayed verbatim from
    /// the manifest so a host editor can render the widget's option surface. Empty for a tile that
    /// declared none. Serialized as the same `{id,label,scope,path,control,choices?,default?}` shape a
    /// built-in view's `options[]` uses, so one vocabulary drives the editor.
    pub options: Vec<lb_assets::ExtUiOption>,
}

/// The `dashboard.catalog` response — the merged authoring vocabulary for `ws`.
#[derive(Debug, Clone, Serialize)]
pub struct WidgetCatalog {
    /// The catalog schema version (from `widget_catalog.json`'s `v`).
    pub v: u64,
    /// The built-in views with their per-view config-field schema (verbatim from the catalog file).
    pub views: Vec<Value>,
    /// The workspace's installed extension `[[widget]]` tiles (workspace-scoped, opaque ids).
    #[serde(rename = "extWidgets")]
    pub ext_widgets: Vec<ExtWidget>,
    /// The genui component names this node can render (names-only in Slice A).
    #[serde(rename = "genuiComponents")]
    pub genui_components: Vec<String>,
}

/// Read the widget catalog for `ws` as `principal`. The workspace is the caller's (derived from the
/// token, never the request), so the ext-tile fold is workspace-isolated. Denials are opaque.
///
/// The flow: gate the verb, read the static built-in palette, fold the caller's installed `[[widget]]`
/// tiles from `ext.list` (generic — ids opaque), and append the genui component names. The built-in
/// view set is workspace-independent; only the ext tiles vary by workspace.
pub async fn dashboard_catalog(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
) -> Result<WidgetCatalog, ToolError> {
    // Gate the verb itself first — without `mcp:dashboard.catalog:call` the catalog denies opaquely.
    authorize_tool(principal, ws, "dashboard.catalog")?;

    let doc: Value = serde_json::from_str(CATALOG_JSON).expect("widget_catalog.json is valid JSON");
    let v = doc.get("v").and_then(Value::as_u64).unwrap_or(0);
    let views = doc
        .get("views")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    // Fold the workspace's installed extension `[[widget]]` tiles — the generic `ext.list` seam, ids
    // treated as opaque data (rule 10). `ext.list` gates on `mcp:ext.list:call`; a caller without it
    // simply sees no ext tiles (the built-in palette is unaffected), not a hard deny of the catalog.
    let ext_widgets = match ext_list(node, principal, ws).await {
        Ok(rows) => rows
            .into_iter()
            .flat_map(|row| {
                let ext = row.ext;
                row.widgets.into_iter().map(move |w| ExtWidget {
                    ext: ext.clone(),
                    // The view-key segment is the tile's stable id (ext-widget-panel-options scope) —
                    // the resolved `ExtUi::id` (populated at install as `id` or `slug(label)`), falling
                    // back to slugging the label for any legacy install written before the id field.
                    // This is what an author composes into `view:"ext:<ext>/<widget>"`, and it MUST
                    // match the UI's `widgetIdOf` slug so picker, renderer, and catalog agree.
                    widget: w
                        .id
                        .clone()
                        .unwrap_or_else(|| lb_ext_loader::slug(&w.label)),
                    label: w.label,
                    icon: w.icon,
                    data: w.data,
                    scope: w.scope,
                    options: w.options,
                })
            })
            .collect(),
        Err(_) => Vec::new(),
    };

    Ok(WidgetCatalog {
        v,
        views,
        ext_widgets,
        genui_components: super::genui::genui_component_names(),
    })
}

/// The `dashboard.catalog` descriptor — a no-arg read verb, so `tools.catalog` self-describes it (and
/// the palette can offer it). Mirrors how `tools.catalog` itself is listed: a named host-native verb.
pub fn catalog_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        emits_external: false,
        name: "dashboard.catalog".to_string(),
        title: "Widget catalog".to_string(),
        group: "dashboard".to_string(),
        input_schema: Some(serde_json::json!({ "type": "object", "properties": {} })),
        result: None,
    }
}
