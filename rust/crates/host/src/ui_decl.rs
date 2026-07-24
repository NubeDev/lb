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

use lb_assets::{ExtNavItem, ExtUi, ExtUiOption};
use lb_ext_loader::{Manifest, NavItem, Widget, WidgetOption};

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

/// Intersect a declared `scope` against the granted caps (a declared tool survives only if
/// `mcp:<tool>:call` is in `granted`) — the "gated caller, never a trusted decider" rule.
fn narrow_scope(scope: &[String], granted: &[String]) -> Vec<String> {
    scope
        .iter()
        .filter(|t| granted.iter().any(|g| g == &format!("mcp:{t}:call")))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use lb_ext_loader::Manifest;

    const TOML: &str = r#"
[extension]
id = "x"
version = "0.1.0"
[runtime]
tier = "wasm"
world = "lazybones:ext/extension@0.1.0"
placement = "either"
[ui]
entry = "p.mjs"
label = "Page"
scope = ["series.find", "series.latest"]
[[widget]]
entry = "a.mjs"
label = "A"
scope = ["series.latest"]
data = true
options = [
  { id = "band", label = "Comfort band", scope = "options", path = "band", control = "number", default = 1.5 },
]
[[widget]]
entry = "b.mjs"
label = "B"
scope = ["series.read"]
[visibility]
class = "private"
"#;

    #[test]
    fn projects_page_and_every_widget() {
        let m = Manifest::parse(TOML).unwrap();
        let granted = vec![
            "mcp:series.find:call".to_string(),
            "mcp:series.latest:call".to_string(),
            "mcp:series.read:call".to_string(),
        ];
        let (page, widgets) = project(&m, &granted);
        assert_eq!(page.unwrap().label, "Page");
        assert_eq!(widgets.len(), 2);
        assert_eq!(widgets[0].label, "A");
        assert_eq!(widgets[1].label, "B");
    }

    #[test]
    fn data_flag_projects_and_defaults_false() {
        // Widget A opts into frames-in (`data = true`); widget B omits it (defaults false); a page is
        // never a data view. The flag carries through `project`/`narrow` onto the durable `ExtUi`.
        let m = Manifest::parse(TOML).unwrap();
        let granted = vec![
            "mcp:series.find:call".to_string(),
            "mcp:series.latest:call".to_string(),
            "mcp:series.read:call".to_string(),
        ];
        let (page, widgets) = project(&m, &granted);
        assert!(!page.unwrap().data, "a page is never a data view");
        assert!(widgets[0].data, "widget A declared data = true");
        assert!(!widgets[1].data, "widget B omitted data → false");
    }

    #[test]
    fn widget_id_and_options_carry_through() {
        // ext-widget-panel-options scope: the resolved id (slug of the label when absent) + the
        // declarative options are relayed verbatim onto the durable ExtUi; a page carries neither.
        let m = Manifest::parse(TOML).unwrap();
        let granted = vec![
            "mcp:series.latest:call".to_string(),
            "mcp:series.read:call".to_string(),
        ];
        let (page, widgets) = project(&m, &granted);
        let page = page.unwrap();
        assert_eq!(page.id, None, "a page has no widget id");
        assert!(page.options.is_empty(), "a page carries no options");
        assert_eq!(
            widgets[0].id.as_deref(),
            Some("a"),
            "id defaults to slug(label)"
        );
        assert_eq!(widgets[0].options.len(), 1);
        assert_eq!(widgets[0].options[0].id, "band");
        assert_eq!(widgets[0].options[0].default, Some(serde_json::json!(1.5)));
        assert!(
            widgets[1].options.is_empty(),
            "widget B declared no options"
        );
    }

    #[test]
    fn projects_ui_nav_onto_page_not_widget() {
        // ext-nav-contribution scope: a page's `[[ui.nav]]` items are relayed verbatim onto its durable
        // ExtUi.nav; a widget carries none. The host is a relay — flags + order + ids come through as-is.
        const NAV_TOML: &str = r#"
[extension]
id = "ems"
version = "0.1.0"
[runtime]
tier = "wasm"
world = "lazybones:ext/extension@0.1.0"
placement = "either"
[ui]
entry = "p.mjs"
label = "EMS"
[[ui.nav]]
id = "sites"
label = "nav.sites"
icon = "layout-grid"
dynamic = true
[[ui.nav]]
id = "studio"
label = "nav.studio"
admin = true
[[ui.nav]]
id = "fleet"
label = "nav.fleet"
dashboard = "dashboard:ems-fleet-overview"
vars = { site = "site-1" }
[[widget]]
entry = "w.mjs"
label = "Tile"
[visibility]
class = "private"
"#;
        let m = Manifest::parse(NAV_TOML).unwrap();
        let (page, widgets) = project(&m, &[]);
        let page = page.unwrap();
        assert_eq!(page.nav.len(), 3);
        assert_eq!(page.nav[0].id, "sites");
        assert_eq!(page.nav[0].label, "nav.sites");
        assert_eq!(page.nav[0].icon, "layout-grid");
        assert!(page.nav[0].dynamic && !page.nav[0].admin);
        // An ext-route item carries no dashboard/vars.
        assert!(page.nav[0].dashboard.is_none() && page.nav[0].vars.is_empty());
        assert_eq!(page.nav[1].id, "studio");
        assert!(page.nav[1].admin && !page.nav[1].dynamic);
        // ext-dashboard-nav scope: a static dashboard nav item's dashboard/vars relay verbatim.
        assert_eq!(page.nav[2].id, "fleet");
        assert_eq!(
            page.nav[2].dashboard.as_deref(),
            Some("dashboard:ems-fleet-overview")
        );
        assert_eq!(page.nav[2].vars.get("site").map(String::as_str), Some("site-1"));
        assert!(widgets[0].nav.is_empty(), "a widget contributes no nav");
    }

    #[test]
    fn narrows_scope_to_grant() {
        // Only series.latest is granted — the page's series.find must be dropped, the b-widget's
        // series.read too (it is not granted).
        let m = Manifest::parse(TOML).unwrap();
        let granted = vec!["mcp:series.latest:call".to_string()];
        let (page, widgets) = project(&m, &granted);
        assert_eq!(page.unwrap().scope, vec!["series.latest".to_string()]);
        assert_eq!(widgets[0].scope, vec!["series.latest".to_string()]);
        assert!(widgets[1].scope.is_empty());
    }
}
