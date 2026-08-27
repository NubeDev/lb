//! The dashboard RECORD (dashboard scope, "Data") — the page and everything about it except the
//! cells themselves, which live in [`super::cell`] and [`super::binding`] and are re-exported below.
//! A dashboard is an **asset**: a workspace-namespaced `dashboard:{id}` record holding the grid
//! layout (`cells[]`), the owner, and the S4 visibility tier. Sharing to a *team* is a `share` EDGE
//! (reused from `lb_assets`), not a field — so the existing three-gate read check applies unchanged
//! (dashboard scope, "How it fits").
//!
//! `cells` is a typed nested object (queryable, no app-side JSON parsing) — the storage discipline
//! the ingest scope established. The binding is the forever-contract Phase 2 moves behind the bridge
//! unchanged: a cell names a `widget_type` and a `binding` (explicit series OR a tag-facet query).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::report::ExportProfile;

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
#[derive(Default)]
pub enum Visibility {
    /// Owner only.
    #[default]
    Private,
    /// Shared to a team via the `share` edge (read by team members).
    Team,
    /// Any workspace member with the read cap.
    Workspace,
}

// The CELL shapes live beside this file — `cell.rs` (the grid cell) and `binding.rs` (what it binds
// to) — because they change for panel-model reasons while the record below changes for
// page-settings ones (FILE-LAYOUT). Re-exported here so every `super::model::Cell` path, and the
// barrel in `mod.rs`, keep resolving unchanged.
pub use super::binding::{Action, QueryOptions, Source, Target};
pub use super::cell::Cell;

use super::null_default::null_default;

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
    /// CONDITIONAL variable (conditional-variables scope) — the same `${var} == value` grammar as
    /// [`Target::show_when`], here saying when this variable's CONTROL is shown: a "compare site"
    /// picker that appears only while `comparison` is `Site`. Blank ⇒ always shown. Client-evaluated;
    /// the host stores the expression and never branches on it. Typed for the same reason the
    /// target's twin is: unknown keys do not survive a save.
    #[serde(
        default,
        rename = "showWhen",
        deserialize_with = "null_default",
        skip_serializing_if = "String::is_empty"
    )]
    pub show_when: String,
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
    /// Show a "cached · Ns" freshness chip when the board serves cached reads. Default off.
    #[serde(default, deserialize_with = "null_default")]
    pub cached: bool,
}

/// A dashboard's stored **default time window** (relative-time-range scope) — grammar expressions,
/// not frozen instants: `from: "last-7-days"` still means the last seven days when the board is
/// opened next month. Typed (not an `options` key) because the `Dashboard` struct drops untyped
/// top-level keys on save — the `kind`/`reportIds` reason. VALIDATED on save like `kind` (a stored
/// unresolvable expression is a board that errors on every open), unlike the opaque `width`.
/// `to` empty = the window's own end (a range token IS both ends; an endpoint `from` ends at now).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DashboardTime {
    /// A range token (`today`, `this-month`, `last-3-months`) or an endpoint (`now-4h`,
    /// `now-1d/d`, an ISO day/instant, an epoch ms).
    #[serde(default, deserialize_with = "null_default")]
    pub from: String,
    /// An optional endpoint (exclusive). Empty with a range token (the token is both ends) or to
    /// end at now.
    #[serde(default, deserialize_with = "null_default")]
    pub to: String,
}

/// A dashboard record. The persisted layout + sharing metadata (dashboard scope, "Data").
/// Derives `Default` so the next additive field costs no call-site churn (the `Policy` precedent).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Dashboard {
    /// Stable slug, unique per workspace (the record id `dashboard:{id}`).
    pub id: String,
    pub title: String,
    /// A one-line human subtitle shown under the page title (dashboard page-settings). Additive/
    /// defaulted — a pre-settings dashboard round-trips unchanged; the UI falls back to a default
    /// blurb when empty. Opaque to the host beyond serde.
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
    /// The page's display HEADING — the human name, distinct from the slug-ish `title`. Shown in the
    /// in-body heading block and as the breadcrumb label. Empty ⇒ the UI falls back to `title`.
    ///
    /// Typed for the reason `kind`/`reportIds` are: this struct DROPS unknown top-level keys, so the
    /// client's `heading` vanished on the first save. Opaque to the host beyond serde.
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
    pub heading: String,
    /// How large the in-body heading block renders — `"small" | "medium" | "large"`, empty ⇒ medium.
    /// Additive/defaulted, host-opaque (an unknown value degrades to the UI's default size).
    #[serde(default, deserialize_with = "null_default", rename = "headingSize")]
    pub heading_size: String,
    /// Whether the in-body heading block (icon + heading + description) shows above the first widget
    /// row. Stored as an explicit tri-state via `Option` because ABSENT means *shown* (the default a
    /// pre-heading dashboard must keep) while `Some(false)` is the author's deliberate hide — a bare
    /// `bool` would default to `false` and silently hide every existing board's heading.
    #[serde(
        default,
        rename = "showHeading",
        skip_serializing_if = "Option::is_none"
    )]
    pub show_heading: Option<bool>,
    /// How the dashboard's VARIABLE controls present themselves above the board (dashboard
    /// variable-display) — `"chips"` (the default: compact "＋ Label · value" chip pickers on one
    /// wrapping row), `"bar"` (the classic labelled-select bar), `"inline"` (a single condensed
    /// summary line that expands on click), or `"filters"` (the row collapses behind a counted
    /// "Filters" button in the toolbar). Empty ⇒ chips.
    ///
    /// Additive/defaulted and host-opaque beyond serde: the host neither renders nor validates the
    /// vocabulary (an unknown value degrades to the default presentation and is visible on screen —
    /// the same reasoning that leaves `width` opaque while `kind` is checked).
    #[serde(default, deserialize_with = "null_default", rename = "varsDisplay")]
    pub vars_display: String,
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
    /// re-open serves from the federation result / gateway response cache.
    ///
    /// **Tri-state, and that is the point.** `None` (absent on the wire) means the author never chose
    /// a freshness — the UI applies its default, so caching is ON by default for a new board. `Some(0)`
    /// means the author explicitly chose **live** and is a real, distinct value the UI must honour by
    /// sending no directive. Collapsing the two (a bare `u64` with `#[serde(default)]`, as this once
    /// was) makes every board read as "caching explicitly disabled" and the default unreachable.
    /// `skip_serializing_if` keeps unset ABSENT on the wire so existing boards pick the default up
    /// without a re-save. Additive — a pre-freshness dashboard round-trips unchanged. Opaque to the
    /// host beyond serde (the host caches on the directive the UI sends, not on this field).
    #[serde(default, rename = "cacheTtlS", skip_serializing_if = "Option::is_none")]
    pub cache_ttl_s: Option<u64>,
    /// The stored DEFAULT time window (relative-time-range scope) — see [`DashboardTime`]. Follows
    /// `width`'s four layers exactly (model field, save schema + preserve-on-omit, tool arg,
    /// gateway body field) but is VALIDATED on save like `kind`. `Option` + `skip_serializing_if`
    /// so a pre-time dashboard round-trips byte-clean and absent means "the client default window",
    /// which is distinct from an author's explicit choice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time: Option<DashboardTime>,
    /// Page content width (dashboard page-settings) — `"wide"` (full-bleed, the default/empty) or
    /// `"centered"` (a constrained, centred content column, the marketing-page look). Additive/
    /// defaulted — a pre-width dashboard round-trips as empty (⇒ wide). Opaque to the host beyond
    /// serde; the UI reads it and clamps the board container.
    #[serde(default, deserialize_with = "null_default")]
    pub width: String,
    /// How the board PACKS gaps (dashboard page-settings) — `"none"` (the default/empty: panels stay
    /// exactly where the author put them), `"vertical"` (float them up), `"horizontal"` (float them
    /// left) or `"both"` (float up and left until nothing moves; performed by the UI, which alternates
    /// single-axis passes because its grid library has no two-axis mode). Follows `width`'s four layers exactly (model field, save schema + preserve-on-omit, tool
    /// arg, gateway body field) and is opaque to the host beyond serde for the same reason: the host
    /// stores no geometry opinion, the UI hands the value to its grid.
    ///
    /// Typed rather than left to ride along BECAUSE this struct drops unknown top-level keys — an
    /// untyped `compact` round-trips to nothing on the first save, which is exactly what a UI-only
    /// attempt at this setting produces: a control that appears to work and silently forgets.
    ///
    /// `"none"` is a real value, not an omission, because the save path preserves on omit — an author
    /// turning compaction back OFF has to be able to say so. Empty (every pre-compact dashboard) reads
    /// as `"none"`, so nothing needs migrating.
    #[serde(default, deserialize_with = "null_default")]
    pub compact: String,
    /// What this record IS — see [`super::kind`] for the vocabulary and why it is typed.
    #[serde(default, deserialize_with = "null_default")]
    pub kind: String,
    /// Report-kind dashboard ids this page's **Generate report** control offers (page-settings,
    /// admin-set). One id renders a direct button, several a menu; empty ⇒ no control.
    ///
    /// Typed for the same reason `kind` is: this struct DROPS unknown top-level keys, so an untyped
    /// `reportIds` would vanish on the first save. Opaque to the host beyond serde — it neither
    /// resolves the ids nor gates on them; the launcher's gate is the viewer's ability to READ the
    /// bound report, which the roster answers. A dangling id is therefore not an error here: it
    /// simply does not appear in a roster the viewer can see.
    #[serde(default, deserialize_with = "null_default", rename = "reportIds")]
    pub report_ids: Vec<String>,
    /// The board's saved **export profiles** — named [`ExportOptions`] sets the export dialog offers
    /// (report-pagination-and-export-options scope; see [`ExportProfile`] for why the scope's
    /// "no stored profiles" non-goal was reversed).
    ///
    /// Typed for the same reason `kind`/`reportIds` are, and this is the whole argument: this struct
    /// DROPS unknown top-level keys, so a client-authored `exportProfiles` vanishes on the first save.
    /// There is no other place on the record for it to live.
    ///
    /// **The host never reads a profile.** `report.export` takes no profile id; the client picks one
    /// and sends that profile's `options` on the export call. Opaque beyond serde — no id resolution,
    /// no uniqueness check, no validation of the options until an export actually asks for them.
    ///
    /// Preserve-on-omit like every page setting: an absent key keeps the stored list (a layout save
    /// must not wipe an admin's profiles), an EMPTY array is the explicit clear that gets a board back
    /// to the shipped default. `skip_serializing_if` keeps an empty list off the wire, so a board that
    /// never had profiles round-trips byte-clean.
    #[serde(
        default,
        deserialize_with = "null_default",
        rename = "exportProfiles",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub export_profiles: Vec<ExportProfile>,
    /// The principal who created it (the private→shared model's anchor).
    pub owner: String,
    /// The BARE id of the extension that generates this board (`"modbus"`), or empty for an
    /// ordinary human-authored one (ext-managed-dashboards scope, Goal 2 / D1). Additive/defaulted —
    /// a pre-marker dashboard round-trips byte-clean, no migration.
    ///
    /// **Derived, never accepted as input**: `dashboard.save` computes it from the SAVING PRINCIPAL
    /// in the one helper [`super::managed::managed_by_of`] and preserves it across an update. No verb
    /// reads a `managedBy` argument, so a human cannot mark a board managed and one extension cannot
    /// claim another's. The full principal already lives on `owner` (`"ext:modbus"`); this is the
    /// bare id a badge renders and a roster filter keys on.
    ///
    /// **Opaque to the host** — set and relayed, never interpreted: no lookup of the extension, no
    /// lifecycle coupling (uninstall does NOT cascade-delete its boards, D5), and the host never
    /// branches on WHICH id it holds (rule 10). Clients branch on presence, not value.
    #[serde(default, deserialize_with = "null_default", rename = "managedBy")]
    pub managed_by: String,
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

#[cfg(test)]
mod tests {
    use super::*;

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
