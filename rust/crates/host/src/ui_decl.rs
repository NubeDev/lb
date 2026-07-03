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

use lb_assets::ExtUi;
use lb_ext_loader::Manifest;

/// Build the `(page, widgets)` UI projection for an install from its parsed `manifest` and the
/// computed `granted` cap set. `page` is `Some` iff the manifest declared `[ui]`; `widgets` carries
/// one entry per `[[widget]]` table (empty if none).
pub(crate) fn project(manifest: &Manifest, granted: &[String]) -> (Option<ExtUi>, Vec<ExtUi>) {
    let page = manifest.ui.as_ref().map(|u| {
        // A page is never a data view — `data` stays false.
        narrow(
            &u.scope,
            granted,
            u.entry.clone(),
            u.label.clone(),
            u.icon.clone(),
            false,
        )
    });
    let widgets = manifest
        .widgets
        .iter()
        .map(|w| {
            narrow(
                &w.scope,
                granted,
                w.entry.clone(),
                w.label.clone(),
                w.icon.clone(),
                w.data,
            )
        })
        .collect();
    (page, widgets)
}

/// One `ExtUi` with its declared `scope` intersected against the granted caps (a declared tool
/// survives only if `mcp:<tool>:call` is in `granted`).
fn narrow(
    scope: &[String],
    granted: &[String],
    entry: String,
    label: String,
    icon: String,
    data: bool,
) -> ExtUi {
    let allowed = scope
        .iter()
        .filter(|t| granted.iter().any(|g| g == &format!("mcp:{t}:call")))
        .cloned()
        .collect();
    ExtUi {
        entry,
        label,
        icon,
        scope: allowed,
        data,
    }
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
