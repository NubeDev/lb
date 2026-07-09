//! The dashboard record + cell types (dashboard scope, "Data"). A dashboard is an **asset**: a
//! workspace-namespaced `dashboard:{id}` record holding the grid layout (`cells[]`), the owner, and
//! the S4 visibility tier. Sharing to a *team* is a `share` EDGE (reused from `lb_assets`), not a
//! field — so the existing three-gate read check applies unchanged (dashboard scope, "How it fits").
//!
//! `cells` is a typed nested object (queryable, no app-side JSON parsing) — the storage discipline
//! the ingest scope established. The binding is the forever-contract Phase 2 moves behind the bridge
//! unchanged: a cell names a `widget_type` and a `binding` (explicit series OR a tag-facet query).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Deserialize a defaulted field tolerating an explicit JSON `null` (AI callers emit `"title": null`
/// where a human omits the key — live, two `dashboard.save` turns died on `invalid type: null,
/// expected a string`). `#[serde(default, deserialize_with = "null_default")]` alone only covers the ABSENT key; this covers both.
fn null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Default + serde::Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

/// The table dashboards live in. Record id is `dashboard:{id}` (the id is a stable slug, unique per
/// workspace).
pub const TABLE: &str = "dashboard";

/// Our panel-model document version (viz panel-model scope), pinned on [`Dashboard::schema_version`]
/// at save. `3` = the Grafana-aligned panel model (v3 cells: `sources[]`/`fieldConfig`/
/// `transformations`). Bumped only when the stored *document* shape changes (not when `Cell.v` does).
pub const SCHEMA_VERSION: u32 = 3;

/// A dashboard's visibility tier — the S4 asset-sharing tiers (dashboard scope, "Access").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    /// Owner only.
    Private,
    /// Shared to a team via the `share` edge (read by team members).
    Team,
    /// Any workspace member with the read cap.
    Workspace,
}

impl Default for Visibility {
    fn default() -> Self {
        Visibility::Private
    }
}

/// A cell's data source, v2: ANY MCP tool call (read or write) in the install grant — not the
/// frozen four series verbs (widget-builder scope, "The widget contract, v2"). The forwardable set
/// is `cell.tools ∩ install-grant`, re-checked at the host per call. A v1 cell carries no `source`
/// and falls back to `binding`; a v2 cell names `{ tool, args }`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Source {
    /// The MCP tool the cell reads (or, for a control, the read it reflects). E.g. `series.read`,
    /// `series.watch`, `<ext>.<verb>`.
    pub tool: String,
    /// The arguments passed to `tool` (opaque to the host; re-checked per call).
    #[serde(default, deserialize_with = "null_default")]
    pub args: Value,
}

/// A control's write action, v2: the tool a `switch`/`slider`/`button` CALLS on interaction
/// (widget-builder scope, "Control views"). `args_template` is a typed template with one `{{value}}`
/// slot the interaction fills (the slider value, the switch state). The write tool is gated by its
/// own existing capability, re-checked at the host per call — the cell invents no new cap.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Action {
    /// The write tool invoked on interaction. E.g. `mqtt.publish`, `ingest.write`, `<ext>.<verb>`.
    pub tool: String,
    /// The argument template; a `{{value}}` token (any string leaf) is substituted with the control
    /// state on interaction. Opaque to the host.
    #[serde(default, deserialize_with = "null_default")]
    pub args_template: Value,
}

/// A v3 **target** — a Grafana "target": one query against one datasource (viz panel-model scope).
/// Generalizes the single [`Source`] to an ordered `sources[]`; each carries a `ref_id` (A, B, …)
/// referenced by transformations + overrides, and an optional `datasource` ref. A v2 single-`source`
/// cell reads as `sources[0]` through the UI adapter; the host stores whatever the client sends. The
/// datasource ref is opaque `Value` here — the host does not interpret it (datasource-binding scope
/// owns its resolution, leashed by the target tool's cap ∩ grant, re-checked per call).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Target {
    /// `"A"` | `"B"` | … — referenced by transformations + field overrides.
    #[serde(default, deserialize_with = "null_default", rename = "refId")]
    pub ref_id: String,
    /// Which datasource (native | series | federation | ext). Opaque to the host.
    #[serde(default, deserialize_with = "null_default")]
    pub datasource: Value,
    /// The resolved MCP tool (`store.query` | `series.read` | `federation.query` | ext tool).
    #[serde(default, deserialize_with = "null_default")]
    pub tool: String,
    /// The query args (opaque; re-checked per call, exactly like [`Source::args`]).
    #[serde(default, deserialize_with = "null_default")]
    pub args: Value,
    /// Skip this target's data (Grafana parity).
    #[serde(default, deserialize_with = "null_default")]
    pub hide: bool,
}

/// One grid cell: react-grid-layout geometry + the widget it hosts + its data binding (dashboard
/// scope, "Data").
///
/// **v1 (frozen):** `widget_type` + `binding` (`{series}` | `{find:{tags}}`) + `options`.
/// **v2 (widget-builder scope):** adds `view` (the render vocabulary), `source` (`{tool,args}` — any
/// granted tool, read or write), and `action` (a control's write tool).
/// **v3 (viz panel-model scope):** adds the Grafana-aligned panel shape — `description`, `sources[]`
/// (targets, superseding the single `source`), `transformations[]` (a client-side pipeline, opaque
/// here), `field_config` (per-field option defaults + overrides, opaque here — the UI owns the typed
/// shape and the user-prefs render bridge), and `plugin_version` (import/export round-trip fidelity).
/// All v2/v3 fields are serde-defaulted so a v1 series cell deserializes unchanged (a v1 cell is a v2
/// cell whose tool set is the four read verbs; a v2 cell is a v3 cell with one target + empty
/// field-config). The receiver rejects an unknown major `v`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Cell {
    /// react-grid-layout item key (stable per cell).
    pub i: String,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    /// Contract version. Absent/`0`/`1` = a v1 series cell; `2` = a v2 tool-bound cell.
    #[serde(default, deserialize_with = "null_default")]
    pub v: u32,
    /// Phase 1 built-ins: `chart` | `stat` | `gauge`. Phase 2 adds `ext:<id>` (federated widgets).
    /// Serde-defaulted like every other v-specific field: a v2+/v3 cell is `view`-addressed and has
    /// no `widget_type` — requiring it made the live agent's first honest `dashboard.save` fail with
    /// `missing field widget_type` on cells the catalog itself taught it to build.
    #[serde(default, deserialize_with = "null_default")]
    pub widget_type: String,
    /// A human title for the cell (widget-config-vars scope, Slice 1). Additive `#[serde(default, deserialize_with = "null_default")]` so a
    /// pre-title cell round-trips unchanged; `dashboard.save`/`get` carry it with no new verb. The header
    /// renders it, falling back to a derived label when empty.
    #[serde(default, deserialize_with = "null_default")]
    pub title: String,
    /// v2 render vocabulary: `chart`/`stat`/`gauge`/`table` (read), `plot`/`d3`/`template` (scripted,
    /// iframe), `switch`/`slider`/`button` (controls), `ext:<id>/<widget>` (extension tiles). Empty on
    /// a v1 cell — `widget_type` is authoritative there.
    #[serde(default, deserialize_with = "null_default")]
    pub view: String,
    /// The data binding — `{ "series": "cooler.temp" }` or `{ "find": { "tags": [...] } }`. v1; a v2
    /// cell uses `source` instead (this stays for v1 compatibility).
    #[serde(default, deserialize_with = "null_default")]
    pub binding: Value,
    /// v2 source: the `{ tool, args }` the cell reads/streams. Empty on a v1 cell.
    #[serde(default, deserialize_with = "null_default")]
    pub source: Source,
    /// v2 action: a control's write `{ tool, args_template }`. Empty on a non-control cell.
    #[serde(default, deserialize_with = "null_default")]
    pub action: Action,
    /// Widget-type-specific options (range, unit label, thresholds, inline template code). Opaque to
    /// the host.
    #[serde(default, deserialize_with = "null_default")]
    pub options: Value,
    /// v3 panel description (Grafana parity). Empty on a v1/v2 cell.
    #[serde(default, deserialize_with = "null_default")]
    pub description: String,
    /// v3 targets — supersedes the single `source`. `sources[0]` === `source` for v2 compat (the UI
    /// adapter maps a v2 single-`source` cell to a one-element `sources`). Empty on a v1/v2 cell.
    #[serde(default, deserialize_with = "null_default")]
    pub sources: Vec<Target>,
    /// v3 client-side transformation pipeline (transformations scope). Opaque to the host (the UI
    /// owns the typed `{ id, options, disabled, filter }` shape). Bounded by `save` (record growth).
    #[serde(default, deserialize_with = "null_default")]
    pub transformations: Vec<Value>,
    /// v3 `fieldConfig { defaults, overrides[] }` — per-field option defaults + per-field overrides
    /// (field-config scope: unit/decimals/min-max/thresholds/mappings/color). Opaque to the host;
    /// the UI owns the typed shape AND the user-prefs render bridge. Bounded by `save`.
    #[serde(default, deserialize_with = "null_default", rename = "fieldConfig")]
    pub field_config: Value,
    /// v3 plugin version, for import/export round-trip fidelity. Empty on a v1/v2 cell.
    #[serde(default, deserialize_with = "null_default", rename = "pluginVersion")]
    pub plugin_version: String,
    /// **Library-panel reference** (library-panels scope). When non-empty (`panel:{id}`) this cell is
    /// a *ref cell*: it carries only layout + the ref + bounded per-placement overrides (the `title`
    /// override above and [`Cell::panel_vars`]), and NO spec. `dashboard.get` hydrates the spec from
    /// the `panel` record at read time (host-side), keeping this marker so the editor can offer
    /// link/unlink. The ref is authoritative — a stale hydrated spec echoed back on `save` is ignored.
    /// Empty (the default) = an inline cell, unchanged. Additive `#[serde(default, deserialize_with = "null_default")]` so inline and ref
    /// cells coexist by design.
    #[serde(default, deserialize_with = "null_default", rename = "panelRef")]
    pub panel_ref: String,
    /// Per-placement variable bindings for a ref cell (library-panels scope, the bounded override set:
    /// title + variable bindings). Opaque `Value` (a `{ name: value }` map); applied over the panel's
    /// own variable defaults at hydration. Empty on an inline cell or a ref with no overrides.
    #[serde(default, deserialize_with = "null_default", rename = "panelVars")]
    pub panel_vars: Value,
    /// Set by `dashboard.get` hydration when a ref cell's `panel_ref` cannot be resolved (deleted,
    /// unshared, or unreadable by the viewer) — the cell renders an honest "panel not accessible"
    /// placeholder, never a leaked spec (library-panels scope, "Dangling refs"). Never persisted:
    /// `#[serde(skip_serializing_if)]` keeps it off the stored record and `dashboard.save` ignores it.
    #[serde(default, rename = "panelMissing", skip_serializing_if = "is_false")]
    pub panel_missing: bool,
}

/// serde `skip_serializing_if` helper — keeps a `false` [`Cell::panel_missing`] off the wire/record.
fn is_false(b: &bool) -> bool {
    !*b
}

/// A dashboard VARIABLE definition (widget-config-vars scope, Slice 2). One model: a `name` bound to a
/// resolver — `query`/`source` resolve over a granted `{tool,args}` (rows → options), the static forms
/// (`custom`/`text`/`const`/`interval`) carry their own value. The host stores the DEFINITIONS only; the
/// per-viewer SELECTION lives in the URL (`?var-<name>=`), never on the record. All fields are
/// serde-defaulted so a pre-variables dashboard deserializes unchanged; `dashboard.save`/`get` round-trip
/// it with no new verb. Opaque to the host beyond serde — the resolver tool is leashed by the dashboard's
/// tool set ∩ grant and re-checked at the host per call (rule 5), exactly like a cell source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Variable {
    /// The reference name — `$name` / `${name}` / `[[name]]`.
    pub name: String,
    /// A human label for the bar dropdown (defaults to `name` in the UI).
    #[serde(default, deserialize_with = "null_default")]
    pub label: String,
    /// The resolver kind: `query` | `custom` | `text` | `const` | `interval` | `source`.
    #[serde(default, deserialize_with = "null_default")]
    pub r#type: String,
    /// `query`/`source`: the resolver `{ tool, args }` (opaque; re-checked per call).
    #[serde(default, deserialize_with = "null_default")]
    pub query: Value,
    /// `custom`: a static option list.
    #[serde(default, deserialize_with = "null_default")]
    pub custom: Vec<String>,
    /// `text`: a free-textbox default.
    #[serde(default, deserialize_with = "null_default")]
    pub text: String,
    /// `const`: a hidden fixed value.
    #[serde(default, rename = "const")]
    pub const_: String,
    /// `interval`: a duration list (feeds `$__interval`).
    #[serde(default, deserialize_with = "null_default")]
    pub interval: Vec<String>,
    /// Selection affordances.
    #[serde(default, deserialize_with = "null_default")]
    pub multi: bool,
    #[serde(default, rename = "includeAll")]
    pub include_all: bool,
    /// reusable-pages scope: marks this variable a **page parameter**. A `required` variable left
    /// unbound (no `?var-` URL value, no default) makes the dashboard render the honest "select a
    /// `<label>`" gate (`RequiredVarGate`) instead of firing cells with a `$name`-literal query. This
    /// is what turns an ordinary dashboard into a *template* — no new record type, just a flag.
    /// Additive `#[serde(default, deserialize_with = "null_default")]` — a pre-reusable-pages dashboard round-trips unchanged.
    #[serde(default, deserialize_with = "null_default")]
    pub required: bool,
}

/// A dashboard record. The persisted layout + sharing metadata (dashboard scope, "Data").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dashboard {
    /// Stable slug, unique per workspace (the record id `dashboard:{id}`).
    pub id: String,
    pub title: String,
    /// The principal who created it (the private→shared model's anchor).
    pub owner: String,
    #[serde(default, deserialize_with = "null_default")]
    pub visibility: Visibility,
    #[serde(default, deserialize_with = "null_default")]
    pub cells: Vec<Cell>,
    /// Variable definitions (widget-config-vars scope, Slice 2). Additive `#[serde(default, deserialize_with = "null_default")]` — a
    /// pre-variables dashboard round-trips unchanged. The selection lives in the URL, not here.
    #[serde(default, deserialize_with = "null_default")]
    pub variables: Vec<Variable>,
    /// OUR panel-model document version (viz panel-model scope) — pinned at save, read by the
    /// import/export + migration path. Distinct from `Cell.v` (the cell *contract* version): this
    /// versions the stored *document shape*, that versions what a bridge accepts. Additive/defaulted;
    /// NOT Grafana's `schemaVersion` (that lives only in the interchange JSON, consumed by the mapper).
    #[serde(default, rename = "schemaVersion")]
    pub schema_version: u32,
    pub updated_ts: u64,
    /// Tombstone (soft-delete, §6.8 idempotent). A deleted dashboard is hidden from `list`/`get`.
    #[serde(default, deserialize_with = "null_default")]
    pub deleted: bool,
}

/// The cheap roster row `list` returns — id/title/visibility/updated_ts, **no cell bodies** (the
/// roster stays cheap; dashboard scope, "Get / list").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DashboardSummary {
    pub id: String,
    pub title: String,
    pub visibility: Visibility,
    pub updated_ts: u64,
}

impl From<&Dashboard> for DashboardSummary {
    fn from(d: &Dashboard) -> Self {
        Self {
            id: d.id.clone(),
            title: d.title.clone(),
            visibility: d.visibility,
            updated_ts: d.updated_ts,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A model-authored cell with explicit `null`s (the live agent's shape — two `dashboard.save`
    /// turns died on `invalid type: null, expected a string`) deserializes to the same defaults an
    /// absent key gets.
    #[test]
    fn cell_tolerates_explicit_nulls() {
        let cell: Cell = serde_json::from_value(serde_json::json!({
            "i": "c1", "x": 0, "y": 0, "w": 6, "h": 4, "v": 3,
            "view": "timeseries",
            "widget_type": null,
            "title": null,
            "options": null,
            "sources": null,
            "fieldConfig": null,
            "panelRef": null
        }))
        .expect("nulls deserialize as defaults");
        assert_eq!(cell.view, "timeseries");
        assert_eq!(cell.widget_type, "");
        assert_eq!(cell.title, "");
        assert!(cell.sources.is_empty());
        assert_eq!(cell.panel_ref, "");
    }
}
