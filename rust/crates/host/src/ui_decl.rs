//! Project a manifest's `[ui]`/`[[widget]]` contributions onto the durable [`ExtUi`] records stored
//! on the [`Install`] (ui-federation + dashboard-widgets scopes). Shared by BOTH install tiers — the
//! wasm `install_extension` and the native `install_native` — so a page/widget surfaces in `ext.list`
//! regardless of how the extension is supervised. A page is independent of the runtime tier
//! (`hello-ui` is wasm; `fleet-monitor` is native): both may ship a page and widgets.
//!
//! The single rule this file enforces: **narrow each declared `scope` to the granted caps** — a
//! page/widget can never claim a tool the admin didn't approve (the "gated caller, never a trusted
//! decider" rule). The bridge re-filters and the host re-checks regardless; this is the durable,
//! narrowed truth `ext.list` reports.
//!
//! The narrowing asks the SAME question the runtime gate asks — *which capability gates this verb?* —
//! via [`gate_tool_for`], rather than assuming every verb is gated by its own namesake cap. Several
//! shipped verbs are deliberately ALIASED onto an existing grant because they are a fan-in of the same
//! authorized read, not a new privilege (`viz.query_batch` → `mcp:viz.query:call`,
//! `series.latest_many` → `mcp:series.latest:call`, `series.rollup.read` → `mcp:series.read:call`,
//! the `federation.*` reads → `mcp:federation.query:call`). No `mcp:viz.query_batch:call` exists in
//! any role bundle, so a namesake-only intersection dropped every such verb from the served scope for
//! EVERY install — and because the shell's bridge builds its allow-set from exactly this list, the
//! page then had its calls refused client-side as `out_of_scope` before they could reach the gate that
//! would have allowed them. That is the "shipped but unusable" state the aliases exist to prevent,
//! reappearing one layer up.

use lb_assets::{ExtConnect, ExtNavItem, ExtQueryBlock, ExtQueryField, ExtUi, ExtUiOption};
use lb_ext_loader::{Manifest, NavItem, QueryBlock, QueryField, Widget, WidgetOption};

/// Project the manifest's `[connect]` block onto its durable `ExtConnect` mirror
/// (ros-datasource-unify scope) — `Some` iff the manifest declared one AND every tool it names is
/// granted. All-or-nothing (not `narrow_scope`'s partial-list filter): a connect kind missing even
/// one of its own verbs (e.g. `list` granted but not `create`) is a broken feature, not a narrowed
/// one, so it's withheld entirely rather than surfaced half-working.
pub(crate) fn project_connect(manifest: &Manifest, granted: &[String]) -> Option<ExtConnect> {
    let c = manifest.connect.as_ref()?;
    let has = |tool: &str| granted.iter().any(|g| g == &format!("mcp:{tool}:call"));
    let tools_ok = has(&c.create_tool)
        && has(&c.list_tool)
        && has(&c.delete_tool)
        && c.probe_tool.as_deref().is_none_or(has)
        && c.update_tool.as_deref().is_none_or(has)
        && c.get_tool.as_deref().is_none_or(has);
    if !tools_ok {
        return None;
    }
    Some(ExtConnect {
        kind: c.kind.clone(),
        label: c.label.clone(),
        icon: c.icon.clone(),
        create_tool: c.create_tool.clone(),
        list_tool: c.list_tool.clone(),
        delete_tool: c.delete_tool.clone(),
        probe_tool: c.probe_tool.clone(),
        update_tool: c.update_tool.clone(),
        get_tool: c.get_tool.clone(),
        open_route: c.open_route.clone(),
    })
}

/// Project the manifest's `[[query]]` blocks onto their durable `ExtQueryBlock` mirrors
/// (panel-datasource-query scope) — cloned through UNCONDITIONALLY, unlike `project_connect`'s
/// all-or-nothing grant check: each block's own `tool` gets the normal per-call `authorize_tool`
/// gate through `viz.query` when a panel actually runs it, so pre-filtering here would only hide a
/// query shape a caller could otherwise discover and still be correctly denied on.
pub(crate) fn project_queries(manifest: &Manifest) -> Vec<ExtQueryBlock> {
    manifest.queries.iter().map(project_query_block).collect()
}

fn project_query_block(q: &QueryBlock) -> ExtQueryBlock {
    ExtQueryBlock {
        id: q.id.clone(),
        label: q.label.clone(),
        tool: q.tool.clone(),
        connection_arg: q.connection_arg.clone(),
        fields: q.fields.iter().map(project_query_field).collect(),
    }
}

fn project_query_field(f: &QueryField) -> ExtQueryField {
    ExtQueryField {
        id: f.id.clone(),
        label: f.label.clone(),
        arg: f.arg.clone(),
        control: f.control.clone(),
        choices: f.choices.clone(),
    }
}

/// Build the `(page, widgets)` UI projection for an install from its parsed `manifest` and the
/// computed `granted` cap set. `page` is `Some` iff the manifest declared `[ui]`; `widgets` carries
/// one entry per `[[widget]]` table (empty if none).
pub(crate) fn project(manifest: &Manifest, granted: &[String]) -> (Option<ExtUi>, Vec<ExtUi>) {
    let page = manifest.ui.as_ref().map(|u| ExtUi {
        // A page is never a data view (`data = false`), carries no widget id/options.
        entry: u.entry.clone(),
        label: u.label.clone(),
        icon: u.icon.clone(),
        scope: narrow_scope(&u.scope, granted),
        data: false,
        id: None,
        options: Vec::new(),
        // The page's declared `[[ui.nav]]` destinations, relayed verbatim (validated at parse) — the
        // shell renders them nested + routes `ext:<ext>/<id>`, branching on no id (ext-nav-contribution).
        nav: u.nav.iter().map(project_nav).collect(),
        sidebar: u.sidebar,
    });
    let widgets = manifest
        .widgets
        .iter()
        .map(|w| project_widget(w, granted))
        .collect();
    (page, widgets)
}

/// One widget → its durable `ExtUi`: scope narrowed to grant, and the stable `id` (resolved to
/// `slug(label)` when absent) + declarative `options` carried through verbatim (ext-widget-panel-
/// options scope — the host relays, never interprets). Storing the RESOLVED id means downstream
/// (`dashboard.catalog`, the picker) reads one canonical key without re-slugging.
fn project_widget(w: &Widget, granted: &[String]) -> ExtUi {
    ExtUi {
        entry: w.entry.clone(),
        label: w.label.clone(),
        icon: w.icon.clone(),
        scope: narrow_scope(&w.scope, granted),
        data: w.data,
        id: Some(w.widget_id()),
        options: w.options.iter().map(project_option).collect(),
        // A widget contributes no top-level nav — nav is a page concern (ext-nav-contribution scope).
        nav: Vec::new(),
        // `sidebar` is a PAGE concern (the nav-slot suppression); a widget has no nav slot to
        // suppress in the first place, so this is always `true` for one.
        sidebar: true,
    }
}

/// A manifest `[[ui.nav]]` item → its persisted `ExtNavItem` mirror — a verbatim copy (opaque relay;
/// the host stores/forwards/routes it, never interprets an id).
fn project_nav(n: &NavItem) -> ExtNavItem {
    ExtNavItem {
        id: n.id.clone(),
        label: n.label.clone(),
        icon: n.icon.clone(),
        admin: n.admin,
        dynamic: n.dynamic,
        // The optional HOST-dashboard target (ext-dashboard-nav scope) — relayed verbatim, interpreted
        // never (rule 10). Absent ⇒ an ext-route item, unchanged.
        dashboard: n.dashboard.clone(),
        vars: n.vars.clone(),
        // The optional heading-override TEMPLATE (nav-context-builtins scope) — relayed verbatim,
        // expanded never; validated + capped at manifest parse.
        title_template: n.title_template.clone(),
    }
}

/// A manifest `WidgetOption` → its persisted `ExtUiOption` mirror — a verbatim copy (opaque relay).
fn project_option(o: &WidgetOption) -> ExtUiOption {
    ExtUiOption {
        id: o.id.clone(),
        label: o.label.clone(),
        scope: o.scope.clone(),
        path: o.path.clone(),
        control: o.control.clone(),
        choices: o.choices.clone(),
        default: o.default.clone(),
    }
}

/// Intersect a declared `scope` against the granted caps — the "gated caller, never a trusted decider"
/// rule. A declared tool survives iff the capability that ACTUALLY GATES it is granted, which is
/// `mcp:<tool>:call` for almost every verb and an alias for the handful the host deliberately folds
/// onto an existing grant ([`gate_tool_for`] is the one place that mapping lives).
///
/// This does NOT widen anything: the surviving verb rides a capability the admin already approved, and
/// the host re-checks that same capability on every call. What it fixes is the opposite failure — an
/// aliased verb being dropped from the served scope even though the caller is fully authorized to
/// call it, which made the page's own bridge refuse it before the gate ever saw it.
fn narrow_scope(scope: &[String], granted: &[String]) -> Vec<String> {
    scope
        .iter()
        .filter(|t| {
            let cap = format!("mcp:{}:call", crate::tool_gate::gate_tool_for(t));
            granted.iter().any(|g| g == &cap)
        })
        .cloned()
        .collect()
}

// The tests live in a sibling file (`ui_decl_tests.rs`) rather than inline: this module's own
// responsibility is the projection, and FILE-LAYOUT caps a file at 400 lines. Same `#[path]` split
// the host's test suites already use — the tests stay a private child module, so they keep reaching
// `pub(crate) project`.
#[cfg(test)]
#[path = "ui_decl_tests.rs"]
mod tests;
