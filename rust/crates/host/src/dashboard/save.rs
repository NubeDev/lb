//! `dashboard.save(id, title, cells)` — one idempotent UPSERT for create+update (dashboard scope,
//! "MCP surface"; a fresh id creates, an existing id updates — not two verbs). Synchronous (one small
//! layout record; not a job). Gated by `mcp:dashboard.save:call`.
//!
//! **Ownership on update:** a save against an existing dashboard is allowed only for its owner
//! UNLESS the caller also holds `mcp:dashboard.save_any:call` (an admin-granted cap, checked second
//! so a non-admin never pays its cost) — an admin needs to *fix* a board they do not own, not only
//! delete it (the asymmetry ext-managed-dashboards D2 closes; `dashboard.delete_any` is the shipped
//! sibling this mirrors verbatim). Create stamps `owner = principal`; `visibility` is set via
//! `dashboard.share`, so save **preserves** the existing visibility (it never silently
//! re-privatizes a shared dashboard).
//!
//! **The managed marker:** on CREATE the record's `managedBy` is derived from the saving principal
//! ([`super::managed::managed_by_of`] — the one helper); on UPDATE it is PRESERVED. It is never read
//! from caller input, so a human cannot mark a board managed and an admin's `save_any` fix cannot
//! blank (or steal) the marker of the extension that generates it.

use lb_auth::Principal;
use lb_mcp::ToolDescriptor;
use lb_store::Store;

use crate::report::ExportProfile;

use super::authorize::authorize_dashboard;
use super::error::DashboardError;
use super::kind::{KIND_DASHBOARD, KIND_REPORT};
use super::managed::managed_by_of;
use super::model::{Cell, Dashboard, DashboardTime, Toolbar, Variable, Visibility};
use super::store::{read_dashboard, write_dashboard};
use super::visibility::may_read_dashboard;

/// The `dashboard.save` descriptor — a real arg schema so a model advertised the verb can FORM the
/// call. Without it (name-only row) the live agent guessed the encoding and sent `cells` as a
/// JSON-encoded STRING five turns in a row (see
/// `docs/debugging/agent/dashboard-save-cells-sent-as-json-string.md`). `cells` is typed
/// `array` loudly; the item shape is described, not fully enumerated — the handler's validators
/// (bounds/views/genui/refs) stay authoritative.
pub fn save_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        emits_external: false,
        name: "dashboard.save".to_string(),
        title: "Create or update a dashboard (idempotent upsert)".to_string(),
        group: "dashboard".to_string(),
        input_schema: Some(serde_json::json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "x-lb": { "label": "Dashboard id", "description": "Fresh id creates; existing id updates (owner-only, or an admin holding dashboard.save_any)" } },
                "title": { "type": "string", "x-lb": { "label": "Title" } },
                "description": { "type": "string", "x-lb": { "label": "Description", "description": "Optional one-line subtitle for the page (omit to keep the existing one)" } },
                "heading": { "type": "string", "x-lb": { "label": "Heading", "description": "Optional display heading for the page — the human name, distinct from the id/title; empty falls back to the title (omit to keep the existing one)" } },
                "headingSize": { "type": "string", "enum": ["small", "medium", "large"], "x-lb": { "label": "Heading size", "description": "Optional in-body heading size: 'small', 'medium' (default) or 'large' (omit to keep the existing one)" } },
                "showHeading": { "type": "boolean", "x-lb": { "label": "Show heading", "description": "Optional: show the in-body heading block (icon + heading + description) above the first widget row; default true (omit to keep the existing one)" } },
                "varsDisplay": { "type": "string", "enum": ["chips", "bar", "inline", "filters"], "x-lb": { "label": "Variables display", "description": "Optional presentation for the dashboard's variable controls: 'chips' (default, compact chip pickers), 'bar' (labelled select bar), 'inline' (one condensed summary line that expands on click) or 'filters' (collapsed behind a counted Filters button in the toolbar) (omit to keep the existing one)" } },
                "icon": { "type": "string", "x-lb": { "label": "Icon", "description": "Optional icon-lib name for the page, e.g. 'activity' (omit to keep the existing one)" } },
                "color": { "type": "string", "x-lb": { "label": "Colour", "description": "Optional CSS accent colour for the page icon (omit to keep the existing one)" } },
                "timezone": { "type": "string", "x-lb": { "label": "Timezone", "description": "Optional dashboard timezone — an IANA name like 'Australia/Sydney' or 'browser' (omit to keep the existing one)" } },
                "cacheTtlS": { "type": "integer", "x-lb": { "label": "Freshness (cache TTL)", "description": "Optional per-dashboard viz.query cache TTL in seconds; 0 = live (caching off, an explicit author choice), omit to keep the existing one. Never set on a board = the client default applies (caching on)." } },
                "time": { "type": "object", "properties": {
                    "from": { "type": "string" },
                    "to": { "type": "string" }
                }, "x-lb": { "label": "Default time range", "description": "Optional default window as RELATIVE expressions, e.g. { from: 'last-7-days' } or { from: 'now-6h' } — a range token (today, yesterday, this-month, last-3-months) in 'from' with 'to' absent, or endpoint pair (now-4h, now-1d/d, ISO day/instant, epoch ms). Validated on save; omit to keep the existing one; { from: '', to: '' } clears" } },
                "width": { "type": "string", "x-lb": { "label": "Page width", "description": "Optional page content width: 'wide' (full-bleed, default) or 'centered' (constrained centred column) (omit to keep the existing one)" } },
                "compact": { "type": "string", "enum": ["none", "vertical", "horizontal", "both"], "x-lb": { "label": "Grid compaction", "description": "Optional grid packing: 'none' (default, panels stay where they are put), 'vertical' (panels float up into empty space), 'horizontal' (panels float left) or 'both' (panels float up and left until nothing moves) (omit to keep the existing one)" } },
                "kind": { "type": "string", "enum": ["dashboard", "report"], "x-lb": { "label": "Kind", "description": "Optional record kind: 'dashboard' (default) or 'report' (a paper-shaped board report.export composes A4 pages from) (omit to keep the existing one)" } },
                "reportIds": { "type": "array", "items": { "type": "string" }, "x-lb": { "label": "Bound reports", "description": "Optional report-kind dashboard ids this page's Generate-report control offers (omit to keep the existing ones)" } },
                "exportProfiles": { "type": "array", "items": { "type": "object", "properties": {
                    "id": { "type": "string" },
                    "name": { "type": "string" },
                    "options": { "type": "object" }
                } }, "x-lb": { "label": "Export profiles", "description": "Optional named report.export option sets the export dialog offers, e.g. [{ id: 'a3-landscape', name: 'A3 landscape', options: { paper: 'a3', orientation: 'landscape' } }] — omit to keep the existing ones, [] clears them" } },
                "toolbar": { "type": "object", "properties": {
                    "dateSelect": { "type": "boolean" },
                    "refreshRate": { "type": "boolean" },
                    "share": { "type": "boolean" },
                    "cached": { "type": "boolean" }
                }, "x-lb": { "label": "Toolbar", "description": "Optional header-chrome flags (all hidden by default): dateSelect, refreshRate, share, cached (omit to keep the existing ones)" } },
                "cells": { "type": "array", "items": { "type": "object" }, "x-lb": { "label": "Cells", "description": "A JSON ARRAY of cell objects (never a JSON-encoded string). Each cell: { i, x, y, w, h, view, title?, sources?, options?, fieldConfig? } — view names come from dashboard.catalog; read an existing dashboard with dashboard.get for a template" } },
                "variables": { "type": "array", "items": { "type": "object" }, "x-lb": { "label": "Variables", "description": "Optional dashboard variables (omit if none)" } },
                "now": { "type": "integer", "x-lb": { "label": "Timestamp", "description": "Logical time of the save — unix epoch seconds" } }
            },
            "required": ["id", "title", "cells", "now"]
        })),
        result: None,
    }
}

/// The page-presentation fields of a save, gathered into ONE struct (dashboard page-settings).
///
/// Every field is `Option<_>` with the same meaning: **`None` preserves** the stored value across the
/// save, `Some` sets it (on create, `None` means the type's empty/default). That is the
/// preserve-on-omit discipline `visibility` established — a layout or variable save carries
/// [`PageMeta::default()`] and can never blank the page chrome.
///
/// This is a struct rather than a positional argument list because the list reached seventeen
/// parameters: at that width every new page setting was a churn of ~10 call sites and a real risk of
/// two same-typed `Option<String>`s being transposed silently. Adding a setting is now one field here
/// plus one line in the constructor below.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PageMeta {
    pub description: Option<String>,
    pub heading: Option<String>,
    pub heading_size: Option<String>,
    pub show_heading: Option<bool>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub timezone: Option<String>,
    pub cache_ttl_s: Option<u64>,
    pub toolbar: Option<Toolbar>,
    /// The default time window (relative-time-range scope). `None` preserves; `Some` sets after
    /// VALIDATION (a bad expression refuses the save); a `Some` with both fields empty CLEARS —
    /// the `reportIds` empty-array precedent (an author must be able to remove the default).
    pub time: Option<DashboardTime>,
    pub width: Option<String>,
    /// Grid compaction — `"none"` (default) | `"vertical"` | `"horizontal"` | `"both"`. `None` preserves.
    pub compact: Option<String>,
    pub vars_display: Option<String>,
    pub kind: Option<String>,
    pub report_ids: Option<Vec<String>>,
    /// The board's saved export profiles. `None` preserves the stored list, `Some` sets it, and
    /// `Some(vec![])` CLEARS — the `reportIds` empty-array precedent, and the reason an admin can
    /// delete their last profile and get back to the shipped default.
    pub export_profiles: Option<Vec<ExportProfile>>,
}

/// Upsert dashboard `id` in `ws` with `title` + `cells`, as `principal`, at logical time `now`.
/// Creates on a fresh id (owner = the principal's `owner_sub` — the human behind a derived agent
/// actor, so an agent-built dashboard belongs to whoever asked; visibility = private); updates an
/// existing one (owner-only). Returns the persisted record.
// Argument count is the explicit dependency list; bundling it into a struct would be a refactor.
#[allow(clippy::too_many_arguments)]
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
    // The common case (layout + variable saves): touch no page-settings field — every one is
    // preserved. The settings dialog is the only writer of icon/colour/subtitle; it calls
    // `dashboard_save_meta` directly.
    dashboard_save_meta(
        store,
        principal,
        ws,
        id,
        title,
        PageMeta::default(),
        cells,
        variables,
        now,
    )
    .await
}

/// Validate a supplied [`Dashboard::kind`]. Empty is legal (and means `"dashboard"`); anything other
/// than the two known kinds is a LOUD refusal rather than a stored typo.
///
/// `width` is deliberately opaque and this is deliberately not, because the two fields fail
/// differently: an unknown `width` degrades to the default layout and is visible on screen, whereas a
/// mistyped `kind` drops the record out of BOTH the dashboards roster and the reports roster — a
/// record that saved "successfully" and then cannot be found anywhere.
fn check_kind(kind: &str) -> Result<(), DashboardError> {
    match kind {
        "" | KIND_DASHBOARD | KIND_REPORT => Ok(()),
        other => Err(DashboardError::BadInput(format!(
            "unknown dashboard kind {other:?} (expected {KIND_DASHBOARD:?} or {KIND_REPORT:?})"
        ))),
    }
}

/// Validate a supplied [`DashboardTime`] through the one grammar ([`crate::timerange`]). Validated
/// like `kind` (not opaque like `width`) because the two fail differently: an unknown `width`
/// degrades visibly on screen, whereas a stored unresolvable expression is a board that errors on
/// every open and a schedule that fails at 03:00 with nobody watching. The refusal names the bad
/// token and the legal set (the grammar's own error).
fn check_time(t: &DashboardTime) -> Result<(), DashboardError> {
    let to = (!t.to.is_empty()).then_some(t.to.as_str());
    crate::timerange::validate(&t.from, to)
        .map_err(|e| DashboardError::BadInput(format!("time: {e}")))
}

/// `dashboard.save` with the page-presentation fields (dashboard page-settings). See [`PageMeta`] for
/// the preserve-on-omit contract every one of them follows. This is the full form; [`dashboard_save`]
/// is the presentation-preserving wrapper every layout/variable caller uses.
#[allow(clippy::too_many_arguments)]
pub async fn dashboard_save_meta(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    title: &str,
    meta: PageMeta,
    mut cells: Vec<Cell>,
    variables: Vec<Variable>,
    now: u64,
) -> Result<Dashboard, DashboardError> {
    authorize_dashboard(principal, ws, "dashboard.save")?;
    if id.is_empty() {
        return Err(DashboardError::BadInput("empty dashboard id".into()));
    }
    if let Some(k) = meta.kind.as_deref() {
        check_kind(k)?;
    }
    // The default time window: validate BEFORE anything is read or written, like `kind` — a bad
    // expression refuses the whole save and leaves the stored value untouched. An all-empty pair is
    // the explicit CLEAR, not an error.
    let time = match &meta.time {
        Some(t) if t.from.is_empty() && t.to.is_empty() => Some(None),
        Some(t) => {
            check_time(t)?;
            Some(Some(t.clone()))
        }
        None => None,
    };
    // Lenient-args normalization BEFORE validation: an AI writer regularly sends `options.genui.ir`
    // as a JSON-encoded string; parse it into the object the validator and renderer expect.
    for cell in &mut cells {
        if cell.view == "genui" {
            if let Some(genui) = cell.options.get_mut("genui") {
                super::genui::normalize_genui_block(genui);
            }
        }
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

    // Preserve owner + visibility + the managed marker across an update; only the owner (or an admin
    // holding `dashboard.save_any`) may update. A tombstoned record is treated as absent — a save
    // with that id resurrects it under the new owner (create).
    // The PREVIOUS record is the preserve-on-omit baseline for every page-settings field at once.
    // Carrying the whole `Dashboard` (rather than a widening tuple of its fields) is what keeps a new
    // setting a one-line change: `prev` already has it.
    let (prev, owner, managed_by, visibility) =
        match read_dashboard(store, ws, id).await?.filter(|d| !d.deleted) {
            Some(existing) => {
                // Owner first, admin override strictly SECOND (`&&` short-circuits, so a non-admin never
                // pays the second check's cost) — the exact shape `dashboard.delete`'s `delete_any` uses.
                // Its own capability, never an ambient "is this caller an admin" role test.
                if existing.owner != principal.owner_sub()
                    && authorize_dashboard(principal, ws, "dashboard.save_any").is_err()
                {
                    return Err(managed_denial(store, principal, ws, &existing).await);
                }
                let owner = existing.owner.clone();
                let managed_by = existing.managed_by.clone();
                let visibility = existing.visibility;
                (existing, owner, managed_by, visibility)
            }
            None => (
                Dashboard::default(),
                principal.owner_sub().to_string(),
                // CREATE — the marker is derived from the saving principal, never from the args.
                managed_by_of(principal).unwrap_or_default(),
                Visibility::Private,
            ),
        };

    let dashboard = Dashboard {
        id: id.to_string(),
        title: title.to_string(),
        // Preserve on omit (None), set on Some — page presentation never gets blanked by a layout save.
        description: meta.description.unwrap_or(prev.description),
        heading: meta.heading.unwrap_or(prev.heading),
        heading_size: meta.heading_size.unwrap_or(prev.heading_size),
        // Both sides are `Option<bool>` but they mean different things: `meta.show_heading` is
        // preserve-on-omit, the stored one is the tri-state (absent ⇒ shown). So `None` in the ARG
        // means "keep whatever the record had", including keeping it absent.
        show_heading: meta.show_heading.or(prev.show_heading),
        icon: meta.icon.unwrap_or(prev.icon),
        color: meta.color.unwrap_or(prev.color),
        timezone: meta.timezone.unwrap_or(prev.timezone),
        // Same tri-state as `show_heading`: the ARG's `None` means "keep what the record had"
        // (including keeping it absent ⇒ the UI's default), while a stored `Some(0)` is the
        // author's explicit "live". `.or` preserves both; `.unwrap_or` would erase the distinction.
        cache_ttl_s: meta.cache_ttl_s.or(prev.cache_ttl_s),
        toolbar: meta.toolbar.unwrap_or(prev.toolbar),
        // Preserve-on-omit over a tri-state, like `cache_ttl_s`: `None` (the arg was absent) keeps
        // whatever the record had — including keeping it absent; the validated `Some(Some(_))`
        // sets; `Some(None)` is the explicit clear.
        time: time.unwrap_or(prev.time),
        width: meta.width.unwrap_or(prev.width),
        compact: meta.compact.unwrap_or(prev.compact),
        vars_display: meta.vars_display.unwrap_or(prev.vars_display),
        // Preserve-on-omit like every other page-settings field: a layout save (which sends no `kind`)
        // must never silently turn a report back into a dashboard.
        kind: meta.kind.unwrap_or(prev.kind),
        report_ids: meta.report_ids.unwrap_or(prev.report_ids),
        // Preserve-on-omit / empty-is-clear, the identical path `report_ids` rides: a layout save
        // carries `None` and keeps the author's profiles; an explicit `[]` is how the last one is
        // deleted.
        export_profiles: meta.export_profiles.unwrap_or(prev.export_profiles),
        owner,
        managed_by,
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

/// The refusal for a save the owner check (and the `save_any` override) rejected — typed when, and
/// only when, it is safe to be (ext-managed-dashboards Goal 5).
///
/// Returns [`DashboardError::ManagedDenied`] with the bare managing extension id iff BOTH hold:
///   1. the board is marked (`managed_by` non-empty), and
///   2. this caller could already **read** it — gates 1+2 passed to get here, and gate 3
///      ([`may_read_dashboard`]) says yes.
///
/// Otherwise the opaque [`DashboardError::Denied`], unchanged. (2) is the no-existence-leak rule:
/// a caller who cannot read a private/team board must not learn from a *refusal* that it exists, let
/// alone who generates it. A caller who CAN read it can fetch `managedBy` with `dashboard.get`
/// anyway, so telling them here adds no information — only a better error.
async fn managed_denial(
    store: &Store,
    principal: &Principal,
    ws: &str,
    existing: &Dashboard,
) -> DashboardError {
    if !existing.managed_by.is_empty()
        && may_read_dashboard(store, principal, ws, existing)
            .await
            .is_ok()
    {
        return DashboardError::ManagedDenied(existing.managed_by.clone());
    }
    DashboardError::Denied
}
