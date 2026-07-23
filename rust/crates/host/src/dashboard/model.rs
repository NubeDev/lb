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
    /// state on interaction. Opaque to the host. `rename = "argsTemplate"` because the entire platform
    /// speaks camelCase on the wire — the UI, `flowBindingOfAction`, every reminder descriptor, the
    /// `dashboard.pin` envelope — exactly like the sibling `Target::ref_id`'s `refId`. Without it a
    /// flow-bound switch/slider lost its `flows.inject` binding on every `dashboard.save`/`get`
    /// (stored `null`, read `undefined`), so the flow-fed-widgets feature read as entirely dead.
    #[serde(default, deserialize_with = "null_default", rename = "argsTemplate")]
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

/// Panel-level query options (viz grafana-parity-backend scope, P1) — the editor's "Query options"
/// block plus Grafana's per-panel time override. Typed (not opaque `Value`) because `viz.query`
/// interprets `timeFrom`/`timeShift` when dispatching targets; the rest ride to the client. All
/// fields additive/null-defaulted; the whole struct is skip-if-default so a pre-P1 cell round-trips
/// byte-stable. Regression pin: before P1 the UI sent this as a top-level cell field and the closed
/// `Cell` struct silently DROPPED it on `dashboard.save`
/// (`docs/debugging/dashboard/query-options-silently-dropped-on-save.md`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct QueryOptions {
    /// Cap on returned points per target (0 = unset; the editor's "Max data points").
    #[serde(default, deserialize_with = "null_default", rename = "maxDataPoints")]
    pub max_data_points: u64,
    /// Minimum bucket interval, a duration string (`"10s"`; empty = unset).
    #[serde(default, deserialize_with = "null_default", rename = "minInterval")]
    pub min_interval: String,
    /// The shipped UI's relative-time field (pre-P1 vocabulary, kept verbatim; empty = unset).
    #[serde(default, deserialize_with = "null_default", rename = "relativeTime")]
    pub relative_time: String,
    /// Grafana panel time override: replaces the range with `[now - timeFrom, now]` (`"6h"`).
    #[serde(default, deserialize_with = "null_default", rename = "timeFrom")]
    pub time_from: String,
    /// Grafana panel time shift: moves BOTH range ends earlier by this duration (`"1d"`).
    #[serde(default, deserialize_with = "null_default", rename = "timeShift")]
    pub time_shift: String,
    /// Display-only (Grafana parity): hide the override badge in the panel header. Never affects
    /// the query.
    #[serde(
        default,
        deserialize_with = "null_default",
        rename = "hideTimeOverride"
    )]
    pub hide_time_override: bool,
}

impl QueryOptions {
    /// True when every field is unset — the skip-serializing predicate (a pre-P1 cell stays
    /// byte-stable on the wire and the record).
    pub fn is_empty(&self) -> bool {
        *self == QueryOptions::default()
    }
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
    /// Resize floor (grid units) — react-grid-layout clamps a widget's resize handle to these so a
    /// widget can't shrink below a legible size. Geometry only, opaque to the host (the client grid
    /// enforces them). Additive/serde-defaulted like every v-specific field, and camelCase on the
    /// wire (`minW`/`minH`) like react-grid-layout's own keys: a cell authored before minimums
    /// existed carries `0`, which the client reads as "no floor" and re-derives its per-view default.
    #[serde(default, deserialize_with = "null_default", rename = "minW")]
    pub min_w: u32,
    #[serde(default, deserialize_with = "null_default", rename = "minH")]
    pub min_h: u32,
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
    /// P1 panel query options (viz grafana-parity-backend scope) — the editor's "Query options"
    /// block + the Grafana time override. Typed because `viz.query` applies `timeFrom`/`timeShift`
    /// when dispatching targets; skip-if-empty so a pre-P1 cell round-trips byte-stable.
    #[serde(
        default,
        deserialize_with = "null_default",
        rename = "queryOptions",
        skip_serializing_if = "QueryOptions::is_empty"
    )]
    pub query_options: QueryOptions,
    /// Transparent panel background (Grafana parity) — renderers honor it UI-side; the host carries
    /// it. Skip-if-false so a pre-P1 cell round-trips byte-stable.
    #[serde(
        default,
        deserialize_with = "null_default",
        skip_serializing_if = "is_false"
    )]
    pub transparent: bool,
    /// Panel links (Grafana `DashboardLink[]`) — opaque to the host (the UI renders them); carried
    /// verbatim for import fidelity. Skip-if-empty (byte-stable pre-P1 records).
    #[serde(
        default,
        deserialize_with = "null_default",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub links: Vec<Value>,
    /// **Panel/row repeat** (Grafana parity — viz grafana-dashboard-fidelity slice 2). The name of the
    /// multi-value variable this panel repeats over (`repeat: "meter"`); the renderer expands one tile
    /// per selected value (bounded "+N more"). Carried opaque here — the host stores the binding, the UI
    /// owns the expansion. Additive/skip-if-empty so a non-repeating cell round-trips byte-stable.
    #[serde(
        default,
        deserialize_with = "null_default",
        skip_serializing_if = "String::is_empty"
    )]
    pub repeat: String,
    /// Repeat layout direction (`"h"` | `"v"`; Grafana `repeatDirection`). Meaningful only with
    /// [`Cell::repeat`]; opaque to the host. Skip-if-empty (byte-stable pre-repeat records).
    #[serde(
        default,
        deserialize_with = "null_default",
        rename = "repeatDirection",
        skip_serializing_if = "String::is_empty"
    )]
    pub repeat_direction: String,
    /// Max repeated tiles per row before wrapping (Grafana `maxPerRow`, horizontal repeat). Opaque to
    /// the host; `0` = unset. Skip-if-zero (byte-stable pre-repeat records).
    #[serde(
        default,
        deserialize_with = "null_default",
        rename = "maxPerRow",
        skip_serializing_if = "is_zero_u32"
    )]
    pub max_per_row: u32,
    /// Set by `dashboard.get` hydration when a ref cell's `panel_ref` cannot be resolved (deleted,
    /// unshared, or unreadable by the viewer) — the cell renders an honest "panel not accessible"
    /// placeholder, never a leaked spec (library-panels scope, "Dangling refs"). Never persisted:
    /// `#[serde(skip_serializing_if)]` keeps it off the stored record and `dashboard.save` ignores it.
    #[serde(default, rename = "panelMissing", skip_serializing_if = "is_false")]
    pub panel_missing: bool,
    /// **Grafana import/export passthrough** (viz import-export scope, Phase 4). A bounded blob of the
    /// unknown Grafana panel fields the mapper did not recognize on import, re-emitted verbatim on
    /// export so a supported dashboard round-trips semantically stable. Opaque to the host and to
    /// every renderer; mapped fields WIN over passthrough on export (passthrough fills only gaps).
    /// Additive/skip-if-empty so a non-imported cell stays byte-stable; `save` bounds its size
    /// ([`crate::dashboard::bounds`], `MAX_GRAFANA_PASSTHROUGH`).
    #[serde(
        default,
        deserialize_with = "null_default",
        rename = "_grafana",
        skip_serializing_if = "Value::is_null"
    )]
    pub grafana_passthrough: Value,
}

/// serde `skip_serializing_if` helper — keeps a `false` [`Cell::panel_missing`] off the wire/record.
fn is_false(b: &bool) -> bool {
    !*b
}

/// serde `skip_serializing_if` helper — keeps an unset (`0`) [`Cell::max_per_row`] off the wire/record.
fn is_zero_u32(n: &u32) -> bool {
    *n == 0
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
    /// An optional bar icon (a stable icon-lib name, e.g. `"map-pin"`) shown before the label
    /// (advanced-variables scope). Opaque to the host — additive/defaulted.
    #[serde(default, deserialize_with = "null_default")]
    pub icon: String,
    /// The resolver kind: `query` | `custom` | `text` | `const` | `interval` | `source` | `datasource`.
    #[serde(default, deserialize_with = "null_default")]
    pub r#type: String,
    /// `query`/`source`: the resolver `{ tool, args }` (opaque; re-checked per call).
    #[serde(default, deserialize_with = "null_default")]
    pub query: Value,
    /// `entity` (entity-data-plane scope, Phase D): the entity→table BINDING an `entity` variable
    /// resolves through — `{ entity, source, table, pk, display, parentFk?, parentVar?, backend? }`.
    /// The client (`entityVar.ts`) COMPILES it to the SAME `{ tool, args }` resolver `query` carries
    /// (`SELECT <pk> AS value, <display> AS text FROM <table>` over `store.query`/`federation.query`),
    /// so the host stays opaque here exactly like `query` — it stores the DEFINITION and re-checks the
    /// resolved tool per call (rule 5). Additive `#[serde(default, deserialize_with = "null_default",
    /// skip_serializing_if = "Value::is_null")]` — a pre-entity dashboard round-trips byte-clean and an
    /// empty binding stays off the wire. **Load-bearing:** without this field the typed `Variable` DROPS
    /// the binding on `dashboard.save`/`get`/`pack.apply`, so an entity var resolves no options and a
    /// meter/site template dashboard renders empty (the same silent-drop class as `queryOptions`/
    /// `argsTemplate` before their fields landed).
    #[serde(
        default,
        deserialize_with = "null_default",
        skip_serializing_if = "Value::is_null"
    )]
    pub entity: Value,
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

    // ── Advanced template variables (advanced-variables scope) ──────────────────────────────────────
    // All additive/defaulted so a pre-advanced dashboard round-trips byte-clean. The host stays opaque:
    // these are definition data the client's resolver/interpolator consume, never host-interpreted.
    /// Resolved/static `{text,value,selected?}` options when text ≠ value (opaque list).
    #[serde(default, deserialize_with = "null_default")]
    pub options: Value,
    /// A literal emitted when "All" is selected instead of expanding every option (`.*`, …).
    #[serde(default, rename = "allValue", deserialize_with = "null_default")]
    pub all_value: String,
    /// A regex applied to each resolved query row (filters + `(?<text>)`/`(?<value>)` capture split).
    #[serde(default, deserialize_with = "null_default")]
    pub regex: String,
    /// Which side of a resolved row the regex applies to: `value` (default) | `text`.
    #[serde(default, rename = "regexApplyTo", deserialize_with = "null_default")]
    pub regex_apply_to: String,
    /// Option sort order (`none` | `alphaAsc` | `alphaDesc` | `numAsc` | `numDesc` | `alphaCiAsc` | `alphaCiDesc`).
    #[serde(default, deserialize_with = "null_default")]
    pub sort: String,
    /// When options re-resolve (`never` | `onLoad` | `onTimeRange`).
    #[serde(default, deserialize_with = "null_default")]
    pub refresh: String,
    /// Bar visibility (`dontHide` | `hideLabel` | `hideVariable`).
    #[serde(default, deserialize_with = "null_default")]
    pub hide: String,

    // ── Grafana-parity P1 (viz grafana-parity-backend scope) ────────────────────────────────────────
    // Additive/defaulted like every field above; host-opaque definition data.
    /// A human description shown in the variable editor / bar tooltip (Grafana parity).
    #[serde(default, deserialize_with = "null_default")]
    pub description: String,
    /// Opt this variable's selection OUT of the URL (`?var-<name>=`) — selection stays session-local.
    #[serde(default, rename = "skipUrlSync", deserialize_with = "null_default")]
    pub skip_url_sync: bool,
    /// multi/select UX flag (Grafana parity): allow a free-typed value beside the resolved options.
    /// Carried opaque until the UI ships it.
    #[serde(
        default,
        rename = "allowCustomValue",
        deserialize_with = "null_default"
    )]
    pub allow_custom_value: bool,
}

/// Toolbar-chrome visibility flags (dashboard toolbar-settings). Each names one optional header
/// control that is **hidden by default** — a clean board shows none of them; an author opts a control
/// in from Page settings. Host-opaque presentation data (additive/defaulted, exactly like `icon`/
/// `color`): the host stores the booleans and never branches on them. A pre-toolbar dashboard
/// deserializes with every flag `false` (all hidden), matching the default-off intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Toolbar {
    /// Show the date-range pickers (`from`/`to`) in the header. Default off.
    #[serde(default, deserialize_with = "null_default", rename = "dateSelect")]
    pub date_select: bool,
    /// Show the auto-refresh-rate control in the header. Default off.
    #[serde(default, deserialize_with = "null_default", rename = "refreshRate")]
    pub refresh_rate: bool,
    /// Show the share button + the private/team/workspace visibility control. Default off.
    #[serde(default, deserialize_with = "null_default")]
    pub share: bool,
}

/// A dashboard record. The persisted layout + sharing metadata (dashboard scope, "Data").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dashboard {
    /// Stable slug, unique per workspace (the record id `dashboard:{id}`).
    pub id: String,
    pub title: String,
    /// A one-line human subtitle shown under the page title (dashboard page-settings). Additive/
    /// defaulted — a pre-settings dashboard round-trips unchanged; the UI falls back to a default
    /// blurb when empty. Opaque to the host beyond serde.
    #[serde(default, deserialize_with = "null_default")]
    pub description: String,
    /// A stable icon-lib name (e.g. `"layout-dashboard"`, `"activity"`) painted in the roster row and
    /// the page header (dashboard page-settings). Opaque to the host — additive/defaulted; the UI
    /// resolves it (with a fallback) and ignores an unknown name.
    #[serde(default, deserialize_with = "null_default")]
    pub icon: String,
    /// An accent colour for the page icon — any CSS colour string (`"#3b82f6"`, `"tomato"`). Opaque
    /// to the host; additive/defaulted (empty = the shell accent).
    #[serde(default, deserialize_with = "null_default")]
    pub color: String,
    /// Optional header-chrome visibility flags (dashboard toolbar-settings). Additive/defaulted — a
    /// pre-toolbar dashboard round-trips with every flag off (all controls hidden). Opaque to the host.
    #[serde(default, deserialize_with = "null_default")]
    pub toolbar: Toolbar,
    /// Dashboard timezone (Grafana parity, P1) — an IANA name (`"Australia/Sydney"`), `"browser"`,
    /// or empty (unset). The record CARRIES the import; the render path resolves via user-prefs
    /// (prefs-wins-at-render — the canonical-in/localized-out doctrine; grafana-parity-backend
    /// scope, open question resolved in the P1 session doc). Opaque to the host beyond serde.
    #[serde(default, deserialize_with = "null_default")]
    pub timezone: String,
    /// Per-dashboard freshness — the `viz.query` cache TTL in seconds (dashboard-query-acceleration
    /// scope §C). The UI resolves the effective TTL (a set auto-refresh interval wins; else this;
    /// else the client default) and threads it as the top-level `cache: {ttl_s}` directive so a warm
    /// re-open serves from the federation result / gateway response cache. `0` = live (no directive).
    /// Additive/defaulted — a pre-freshness dashboard round-trips unchanged. Opaque to the host beyond
    /// serde (the host caches on the directive the UI sends, not on this field).
    #[serde(default, deserialize_with = "null_default", rename = "cacheTtlS")]
    pub cache_ttl_s: u64,
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
    /// Roster affordances (dashboard page-settings) — carried on the cheap summary so the switcher
    /// can paint the icon/colour without a full `get`. Additive/defaulted.
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub color: String,
    pub visibility: Visibility,
    pub updated_ts: u64,
}

impl From<&Dashboard> for DashboardSummary {
    fn from(d: &Dashboard) -> Self {
        Self {
            id: d.id.clone(),
            title: d.title.clone(),
            icon: d.icon.clone(),
            color: d.color.clone(),
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

    /// The advanced-variables fields (icon + regex/sort/refresh/allValue/hide/options + the `datasource`
    /// type) round-trip through `Variable` — the host stores the DEFINITIONS, so a field it drops is a
    /// field the client silently loses on save. Regression for exactly that: the closed struct must carry
    /// every additive field the UI sends.
    #[test]
    fn variable_round_trips_advanced_fields() {
        let sent = serde_json::json!({
            "name": "region",
            "label": "Region",
            "icon": "map-pin",
            "type": "query",
            "query": { "tool": "store.query", "args": { "sql": "SELECT name FROM region" } },
            "multi": true,
            "includeAll": true,
            "allValue": ".*",
            "regex": "(?<text>.+) \\((?<value>[A-Z]+)\\)",
            "regexApplyTo": "value",
            "sort": "alphaAsc",
            "refresh": "onTimeRange",
            "hide": "hideLabel",
            "options": [{ "text": "West", "value": "WST" }],
        });
        let v: Variable = serde_json::from_value(sent.clone()).expect("deserializes");
        assert_eq!(v.icon, "map-pin");
        assert_eq!(v.all_value, ".*");
        assert_eq!(v.regex_apply_to, "value");
        assert_eq!(v.sort, "alphaAsc");
        assert_eq!(v.refresh, "onTimeRange");
        assert_eq!(v.hide, "hideLabel");
        assert_eq!(
            v.options,
            serde_json::json!([{ "text": "West", "value": "WST" }])
        );

        // Re-serialize and confirm every advanced field survives the store round-trip (not dropped).
        let out = serde_json::to_value(&v).expect("serializes");
        assert_eq!(out["icon"], "map-pin");
        assert_eq!(out["allValue"], ".*");
        assert_eq!(out["regexApplyTo"], "value");
        assert_eq!(out["sort"], "alphaAsc");
        assert_eq!(out["refresh"], "onTimeRange");
        assert_eq!(out["hide"], "hideLabel");
        assert_eq!(
            out["options"],
            serde_json::json!([{ "text": "West", "value": "WST" }])
        );
    }

    /// An `entity`-type variable's BINDING round-trips through `Variable` (entity-data-plane Phase D).
    /// The closed struct must carry the binding the UI's `entityVar.ts` compiles its resolver from —
    /// dropping it (the state before this field) makes an entity var resolve NO options, so a
    /// meter/site template dashboard renders empty. This is the serde-level pin; the MCP save→get pin
    /// lives in `tests/dashboard_entity_var_test.rs`.
    #[test]
    fn entity_variable_binding_round_trips() {
        let sent = serde_json::json!({
            "name": "meter",
            "label": "Meter",
            "type": "entity",
            "required": true,
            "entity": {
                "entity": "meter", "source": "ems-readings", "table": "meter",
                "pk": "id", "display": "name", "backend": "store",
            },
        });
        let v: Variable = serde_json::from_value(sent).expect("deserializes");
        assert_eq!(v.r#type, "entity");
        assert!(v.required);
        assert_eq!(v.entity["table"], "meter");
        assert_eq!(v.entity["backend"], "store");

        // The binding survives re-serialization (the store round-trip) verbatim — not dropped.
        let out = serde_json::to_value(&v).expect("serializes");
        assert_eq!(out["entity"]["pk"], "id");
        assert_eq!(out["entity"]["display"], "name");
        assert_eq!(out["type"], "entity");
    }

    /// The additive guard: a variable with NO entity binding keeps `entity` off the wire
    /// (`skip_serializing_if`), so a pre-entity dashboard round-trips byte-clean rather than growing
    /// an `"entity": null` on every variable.
    #[test]
    fn absent_entity_binding_stays_off_the_wire() {
        let v: Variable = serde_json::from_value(serde_json::json!({
            "name": "env", "type": "custom", "custom": ["prod"],
        }))
        .expect("deserializes");
        assert!(v.entity.is_null());
        let out = serde_json::to_value(&v).expect("serializes");
        assert!(
            out.get("entity").is_none(),
            "empty entity stays off the wire"
        );
    }

    /// A dashboard's page-settings fields (`description`/`icon`/`color`) round-trip through the record
    /// AND onto the cheap summary — the host stores the definitions, so a field it drops is a setting
    /// the client silently loses on save. Regression for exactly that.
    #[test]
    fn dashboard_page_settings_round_trip() {
        let sent = serde_json::json!({
            "id": "ops", "title": "Ops", "owner": "sub|u1", "updated_ts": 1,
            "description": "Fleet health at a glance", "icon": "activity", "color": "#3b82f6",
        });
        let d: Dashboard = serde_json::from_value(sent).expect("deserializes");
        assert_eq!(d.description, "Fleet health at a glance");
        assert_eq!(d.icon, "activity");
        assert_eq!(d.color, "#3b82f6");

        let out = serde_json::to_value(&d).expect("serializes");
        assert_eq!(out["description"], "Fleet health at a glance");
        assert_eq!(out["icon"], "activity");
        assert_eq!(out["color"], "#3b82f6");

        // The summary carries icon + colour so the roster paints them without a full get.
        let sum = DashboardSummary::from(&d);
        assert_eq!(sum.icon, "activity");
        assert_eq!(sum.color, "#3b82f6");
    }

    /// A pre-page-settings dashboard (no description/icon/color) still deserializes — the fields
    /// default to empty, never a "missing field" error (additivity).
    #[test]
    fn dashboard_tolerates_pre_page_settings_shape() {
        let d: Dashboard = serde_json::from_value(serde_json::json!({
            "id": "old", "title": "Old", "owner": "sub|u1", "updated_ts": 1
        }))
        .expect("pre-settings shape deserializes");
        assert!(d.description.is_empty());
        assert!(d.icon.is_empty());
        assert!(d.color.is_empty());
    }

    /// The toolbar-chrome flags round-trip through the record (the host stores the definitions, so a
    /// dropped flag is a setting the client silently loses), and a pre-toolbar dashboard deserializes
    /// with every flag `false` (all controls hidden — the default-off intent).
    #[test]
    fn toolbar_round_trips_and_defaults_off() {
        let sent = serde_json::json!({
            "id": "ops", "title": "Ops", "owner": "sub|u1", "updated_ts": 1,
            "toolbar": { "dateSelect": true, "refreshRate": false, "share": true },
        });
        let d: Dashboard = serde_json::from_value(sent).expect("deserializes");
        assert!(d.toolbar.date_select && d.toolbar.share && !d.toolbar.refresh_rate);
        let out = serde_json::to_value(&d).expect("serializes");
        assert_eq!(out["toolbar"]["dateSelect"], true);
        assert_eq!(out["toolbar"]["refreshRate"], false);
        assert_eq!(out["toolbar"]["share"], true);

        // Pre-toolbar shape: no `toolbar` key ⇒ every flag off (hidden by default).
        let old: Dashboard = serde_json::from_value(serde_json::json!({
            "id": "old", "title": "Old", "owner": "sub|u1", "updated_ts": 1
        }))
        .expect("pre-toolbar shape deserializes");
        assert_eq!(old.toolbar, Toolbar::default());
        assert!(!old.toolbar.date_select && !old.toolbar.refresh_rate && !old.toolbar.share);
    }

    /// Every P1 field (grafana-parity-backend scope) round-trips through serde with its camelCase
    /// wire name — `queryOptions` (all six members), `transparent`, `links` on the cell; `timezone`
    /// on the dashboard; `description`/`skipUrlSync`/`allowCustomValue` on a variable. A field the
    /// closed structs drop is user data silently lost on save — the exact shipped bug this P1 fixed.
    #[test]
    fn p1_fields_round_trip() {
        let cell: Cell = serde_json::from_value(serde_json::json!({
            "i": "c1", "x": 0, "y": 0, "w": 6, "h": 4, "v": 3, "view": "timeseries",
            "transparent": true,
            "links": [{ "title": "Runbook", "url": "https://example.com" }],
            "queryOptions": {
                "maxDataPoints": 300, "minInterval": "10s", "relativeTime": "1h",
                "timeFrom": "6h", "timeShift": "1d", "hideTimeOverride": true
            }
        }))
        .expect("deserializes");
        assert!(cell.transparent);
        assert_eq!(cell.links.len(), 1);
        assert_eq!(cell.query_options.max_data_points, 300);
        assert_eq!(cell.query_options.min_interval, "10s");
        assert_eq!(cell.query_options.relative_time, "1h");
        assert_eq!(cell.query_options.time_from, "6h");
        assert_eq!(cell.query_options.time_shift, "1d");
        assert!(cell.query_options.hide_time_override);
        let out = serde_json::to_value(&cell).expect("serializes");
        assert_eq!(out["transparent"], true);
        assert_eq!(out["links"][0]["title"], "Runbook");
        assert_eq!(out["queryOptions"]["maxDataPoints"], 300);
        assert_eq!(out["queryOptions"]["timeFrom"], "6h");
        assert_eq!(out["queryOptions"]["hideTimeOverride"], true);

        let d: Dashboard = serde_json::from_value(serde_json::json!({
            "id": "ops", "title": "Ops", "owner": "sub|u1", "updated_ts": 1,
            "timezone": "Australia/Sydney"
        }))
        .expect("deserializes");
        assert_eq!(d.timezone, "Australia/Sydney");
        assert_eq!(
            serde_json::to_value(&d).expect("serializes")["timezone"],
            "Australia/Sydney"
        );

        let v: Variable = serde_json::from_value(serde_json::json!({
            "name": "region", "type": "custom", "custom": ["west"],
            "description": "Deployment region", "skipUrlSync": true, "allowCustomValue": true
        }))
        .expect("deserializes");
        assert_eq!(v.description, "Deployment region");
        assert!(v.skip_url_sync && v.allow_custom_value);
        let out = serde_json::to_value(&v).expect("serializes");
        assert_eq!(out["description"], "Deployment region");
        assert_eq!(out["skipUrlSync"], true);
        assert_eq!(out["allowCustomValue"], true);
    }

    /// The additive guard: v1/v2/v3 cells (and pre-P1 dashboards/variables) WITHOUT the P1 fields
    /// still deserialize — everything defaults, never a "missing field" error — and the skip
    /// predicates keep the empty defaults OFF the wire, so a pre-P1 record round-trips byte-stable.
    #[test]
    fn p1_fields_default_on_pre_p1_shapes() {
        // v1 (binding), v2 (source), v3 (sources/fieldConfig) — none carry a P1 field.
        for cell_json in [
            serde_json::json!({ "i": "c1", "x": 0, "y": 0, "w": 4, "h": 3,
                "widget_type": "chart", "binding": { "series": "cooler.temp" } }),
            serde_json::json!({ "i": "c2", "x": 0, "y": 0, "w": 4, "h": 3, "v": 2,
                "view": "stat", "source": { "tool": "series.latest", "args": {} } }),
            serde_json::json!({ "i": "c3", "x": 0, "y": 0, "w": 4, "h": 3, "v": 3,
                "view": "timeseries",
                "sources": [{ "refId": "A", "tool": "series.read", "args": {} }],
                "fieldConfig": { "defaults": {}, "overrides": [] } }),
        ] {
            let cell: Cell = serde_json::from_value(cell_json).expect("pre-P1 cell deserializes");
            assert!(!cell.transparent);
            assert!(cell.links.is_empty());
            assert!(cell.query_options.is_empty());
            // Byte-stability: the empty defaults stay off the wire.
            let out = serde_json::to_value(&cell).expect("serializes");
            assert!(out.get("queryOptions").is_none());
            assert!(out.get("transparent").is_none());
            assert!(out.get("links").is_none());
        }

        // Explicit nulls (the AI-caller shape) also land on defaults, not a type error.
        let cell: Cell = serde_json::from_value(serde_json::json!({
            "i": "c4", "x": 0, "y": 0, "w": 4, "h": 3, "v": 3, "view": "stat",
            "queryOptions": null, "transparent": null, "links": null
        }))
        .expect("nulls deserialize as defaults");
        assert!(cell.query_options.is_empty() && !cell.transparent && cell.links.is_empty());

        let d: Dashboard = serde_json::from_value(serde_json::json!({
            "id": "old", "title": "Old", "owner": "sub|u1", "updated_ts": 1
        }))
        .expect("pre-P1 dashboard deserializes");
        assert!(d.timezone.is_empty());

        let v: Variable = serde_json::from_value(serde_json::json!({ "name": "env" }))
            .expect("pre-P1 variable deserializes");
        assert!(v.description.is_empty() && !v.skip_url_sync && !v.allow_custom_value);
    }

    /// Slice-2 additive: panel/row `repeat` round-trips, y-axis `min`/`max` ride the opaque
    /// `fieldConfig` unchanged, and a cell WITHOUT them stays byte-stable (skip predicates).
    #[test]
    fn repeat_and_y_axis_fields_round_trip_and_default_clean() {
        let cell: Cell = serde_json::from_value(serde_json::json!({
            "i": "r1", "x": 0, "y": 0, "w": 6, "h": 4, "v": 3, "view": "timeseries",
            "repeat": "meter", "repeatDirection": "h", "maxPerRow": 3,
            "fieldConfig": { "defaults": { "min": 0, "max": 50, "custom": { "softClamp": true } } }
        }))
        .expect("deserializes");
        assert_eq!(cell.repeat, "meter");
        assert_eq!(cell.repeat_direction, "h");
        assert_eq!(cell.max_per_row, 3);
        // y-axis min/max + soft-clamp ride the opaque fieldConfig untouched (the UI owns the shape).
        assert_eq!(cell.field_config["defaults"]["min"], 0);
        assert_eq!(cell.field_config["defaults"]["max"], 50);
        assert_eq!(cell.field_config["defaults"]["custom"]["softClamp"], true);
        let out = serde_json::to_value(&cell).expect("serializes");
        assert_eq!(out["repeat"], "meter");
        assert_eq!(out["repeatDirection"], "h");
        assert_eq!(out["maxPerRow"], 3);

        // A non-repeating cell keeps every repeat key OFF the wire (byte-stable).
        let plain: Cell = serde_json::from_value(serde_json::json!({
            "i": "p", "x": 0, "y": 0, "w": 4, "h": 3, "v": 3, "view": "stat"
        }))
        .expect("deserializes");
        let out = serde_json::to_value(&plain).expect("serializes");
        assert!(out.get("repeat").is_none());
        assert!(out.get("repeatDirection").is_none());
        assert!(out.get("maxPerRow").is_none());
    }

    /// A PARTIAL `queryOptions` (the shipped UI sends only its trio) deserializes with the rest
    /// defaulted — the struct never demands the P1 additions.
    #[test]
    fn query_options_tolerates_partial_shape() {
        let cell: Cell = serde_json::from_value(serde_json::json!({
            "i": "c1", "x": 0, "y": 0, "w": 6, "h": 4, "v": 3, "view": "timeseries",
            "queryOptions": { "maxDataPoints": 500 }
        }))
        .expect("partial queryOptions deserializes");
        assert_eq!(cell.query_options.max_data_points, 500);
        assert!(cell.query_options.time_from.is_empty());
        assert!(!cell.query_options.hide_time_override);
        assert!(
            !cell.query_options.is_empty(),
            "a set field keeps it on the wire"
        );
    }

    /// A control's `Action` round-trips its `argsTemplate` under the camelCase wire key — the UI, the
    /// reminder descriptors and the `dashboard.pin` envelope all speak `argsTemplate`, so a snake
    /// `args_template` on the wire (the pre-rename bug) dropped a flow-bound switch/slider's
    /// `flows.inject` binding on every save. Pins BOTH directions: `argsTemplate` deserializes into the
    /// struct AND serializes back out as `argsTemplate` (never `args_template`).
    #[test]
    fn action_round_trips_args_template_camel_case() {
        let sent = serde_json::json!({
            "tool": "flows.inject",
            "argsTemplate": { "id": "cooler-ctl", "node": "setpoint-in", "port": "payload", "value": "{{value}}" }
        });
        let a: Action = serde_json::from_value(sent).expect("camelCase argsTemplate deserializes");
        assert_eq!(a.tool, "flows.inject");
        assert_eq!(a.args_template["node"], "setpoint-in");
        assert_eq!(a.args_template["value"], "{{value}}");

        let out = serde_json::to_value(&a).expect("serializes");
        assert_eq!(
            out["argsTemplate"]["node"], "setpoint-in",
            "the wire key is argsTemplate, matching every other producer"
        );
        assert!(
            out.get("args_template").is_none(),
            "the snake key never appears on the wire (the pre-rename bug)"
        );

        // The snake form is NOT accepted on the wire — nothing in lb emits it (grep-verified), so the
        // rename can't strand an existing producer, and this pins that the outlier is closed.
        let snake: Action = serde_json::from_value(serde_json::json!({
            "tool": "flows.inject", "args_template": { "node": "x" }
        }))
        .expect("deserializes (unknown key ignored)");
        assert_eq!(
            snake.args_template,
            Value::Null,
            "a snake args_template is ignored, not read"
        );
    }

    /// A pre-advanced variable (only the original fields) still deserializes — the new fields default,
    /// never a "missing field" error (additivity).
    #[test]
    fn variable_tolerates_pre_advanced_shape() {
        let v: Variable = serde_json::from_value(serde_json::json!({
            "name": "env", "type": "custom", "custom": ["prod", "staging"]
        }))
        .expect("pre-advanced shape deserializes");
        assert_eq!(v.name, "env");
        assert_eq!(v.custom, vec!["prod", "staging"]);
        assert!(v.icon.is_empty());
        assert!(v.regex.is_empty());
        assert_eq!(v.options, Value::Null);
    }
}
