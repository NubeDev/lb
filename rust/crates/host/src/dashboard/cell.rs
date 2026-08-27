//! One grid [`Cell`] — the react-grid-layout geometry plus the widget it hosts and its binding.
//!
//! Split out of `model.rs` (FILE-LAYOUT): `Cell` is the single largest and most-edited shape in the
//! dashboard record, and it changes for panel-model reasons while `Dashboard` changes for
//! page-settings ones. Its binding types live beside it in [`super::binding`]. Re-exported through
//! `model.rs`, so every `super::model::Cell` path still resolves.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::binding::{Action, QueryOptions, Source, Target};
use super::null_default::null_default;

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
    ///
    /// **A TEMPLATE STRING** (nav-context-builtins scope, §G2). It may carry `$var` / `${var}` /
    /// `[[var]]` references — the page's variables plus the `__`-prefixed built-ins (`${__nav.label}`,
    /// `${__page.ext}`, `${__user.login}`, …) — which the CLIENT interpolates at render against the
    /// page's `VarScope`, on the same terms as a panel title. **The host stores it RAW and expands
    /// nothing**, the same posture it holds for [`Action::args_template`]: interpolating server-side
    /// would need the viewer's identity, their URL range and their caps at render time, none of which
    /// the store layer has. An unresolvable reference stays literal (the shipped unknown-variable
    /// rule), so a stored string containing a bare literal `$` — `"Cost $USD per kWh"` — keeps
    /// rendering exactly as it does today. No type change, no migration, no validation added here.
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
    ///
    /// **A TEMPLATE STRING** (nav-context-builtins scope, §G2). It may carry `$var` / `${var}` /
    /// `[[var]]` references — the page's variables plus the `__`-prefixed built-ins (`${__nav.label}`,
    /// `${__page.ext}`, `${__user.login}`, …) — which the CLIENT interpolates at render against the
    /// page's `VarScope`, on the same terms as a panel title. **The host stores it RAW and expands
    /// nothing**, the same posture it holds for [`Action::args_template`]: interpolating server-side
    /// would need the viewer's identity, their URL range and their caps at render time, none of which
    /// the store layer has. An unresolvable reference stays literal (the shipped unknown-variable
    /// rule), so a stored string containing a bare literal `$` — `"Cost $USD per kWh"` — keeps
    /// rendering exactly as it does today. No type change, no migration, no validation added here.
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
    /// **Start a new page before this band** (report-pagination scope). The author's explicit page
    /// break, honoured by [`lb_render::paginate::paginate_with`] ahead of its fit rule.
    ///
    /// It is a MARKER, not a page number, and that is the whole design. `page: u32` would be absolute
    /// on a relative board: insert a panel on page 1 and every later cell's number is stale, so the
    /// record would need a renumbering pass on every edit, and two cells claiming the same page with
    /// incompatible rows would have no defined meaning. A boolean on the band that STARTS a page is
    /// local — it survives dragging, reordering and insertion untouched, and composes with the fit
    /// rule instead of replacing it. (It is also the shape the retired notebook `Block` used.)
    ///
    /// Every cell on one board row is one band, so the marker is read from whichever cell of that row
    /// carries it; setting it on one tile of a KPI row breaks before the whole row.
    ///
    /// Typed, not free-form: the `Dashboard` model drops unknown top-level keys, so an untyped field
    /// would round-trip to nothing on the first save. Skip-if-false so a pre-feature cell serialises
    /// byte-identically to today.
    #[serde(
        default,
        deserialize_with = "null_default",
        rename = "pageBreakBefore",
        skip_serializing_if = "is_false"
    )]
    pub page_break_before: bool,
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
