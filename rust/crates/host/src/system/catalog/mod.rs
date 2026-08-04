//! The **static host-native tool catalog** — the authoritative list of built-in MCP verbs the host
//! dispatches directly (NOT components, so they have no manifest and are not in the runtime
//! `Registry`). `system.tools` appends this to the registry-derived extension tools so the catalog is
//! the *whole* reachable surface, not just the plugin half.
//!
//! It is kept beside the dispatcher it mirrors (`tool_call.rs::is_host_native`): every prefix that
//! file dispatches has at least one entry here (asserted by `host_catalog_covers_dispatch_prefixes`),
//! so a whole verb family cannot silently go missing from the console. The descriptions are
//! hand-written one-liners — source code is the only source of truth for a host verb (it has no
//! manifest to read), so the list lives here as a `const`.
//!
//! **Layout (FILE-LAYOUT §8).** The inventory was one 1394-line table; it is now one file per verb
//! family group, each holding only that group's rows, assembled by [`FAMILIES`] below. A new family
//! is a new file + one line in the `mod`/`FAMILIES` pair — the table never grows past its ratchet
//! again. Nothing here names an extension (rule 10): these are host verbs only.

mod agent;
mod assets;
mod authz;
mod bus;
mod cache;
mod channel;
mod dashboard;
mod datasource;
mod devkit;
mod ext;
mod flows;
mod forms;
mod host;
mod identity;
mod insight;
mod layout;
mod media;
mod nav;
mod notify;
mod panel;
mod prefs;
mod reminder;
mod report;
mod rules;
mod schedule;
mod secret;
mod series;
mod store;
mod system;
mod telemetry;
mod template;
mod timerange;
mod undo;
mod update;
mod versions;
mod viz;
mod workflow;

use super::model::ToolInfo;

/// One static catalog row: the qualified verb, its group (the family prefix), and a one-line summary.
struct HostTool {
    tool: &'static str,
    group: &'static str,
    description: &'static str,
}

/// The built-in host-native verbs, one slice per family group. Mirrors `tool_call.rs::is_host_native`
/// (every prefix there appears in one of these slices) plus the host-native services that route
/// outside that bridge (`system.*` is dispatched by the gateway/UI directly, not the bridge, but it is
/// still a reachable tool). Order is irrelevant — [`host_catalog`] sorts by qualified name.
const FAMILIES: &[&[HostTool]] = &[
    agent::AGENT,
    assets::ASSETS,
    authz::AUTHZ,
    bus::BUS,
    cache::CACHE,
    channel::CHANNEL,
    dashboard::DASHBOARD,
    datasource::DATASOURCE,
    devkit::DEVKIT,
    ext::EXT,
    flows::FLOWS,
    schedule::SCHEDULE,
    forms::FORMS,
    host::HOST,
    identity::IDENTITY,
    insight::INSIGHT,
    layout::LAYOUT,
    media::MEDIA,
    nav::NAV,
    notify::NOTIFY,
    panel::PANEL,
    prefs::PREFS,
    reminder::REMINDER,
    report::REPORT,
    rules::RULES,
    secret::SECRET,
    series::SERIES,
    store::STORE,
    system::SYSTEM,
    telemetry::TELEMETRY,
    template::TEMPLATE,
    timerange::TIMERANGE,
    undo::UNDO,
    update::UPDATE,
    versions::VERSIONS,
    viz::VIZ,
    workflow::WORKFLOW,
];

/// The static host-native catalog as `ToolInfo` rows (`source = "host"`), sorted by qualified name so
/// the page renders a stable order. The extension half (registry-derived) is appended by the caller.
pub(crate) fn host_catalog() -> Vec<ToolInfo> {
    let mut out: Vec<ToolInfo> = FAMILIES
        .iter()
        .flat_map(|f| f.iter())
        .map(|t| ToolInfo {
            tool: t.tool.to_string(),
            description: t.description.to_string(),
            source: "host".to_string(),
            group: t.group.to_string(),
        })
        .collect();
    out.sort_by(|a, b| a.tool.cmp(&b.tool));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every host-native verb-family prefix the dispatcher (`tool_call.rs::is_host_native`) routes has
    /// at least one catalog entry — so a whole family cannot silently vanish from the console OR the
    /// agent's `tools.catalog`-derived menu (which now serves this inventory). Derived from the
    /// dispatcher's OWN shared const, not a hand-copied list — a hand-maintained mirror is exactly how
    /// `datasource.`/`viz.`/`flows.`/… went missing (see
    /// debugging/agent/persona-menu-missing-tools-catalog-descriptor-only.md). `system.` (routed
    /// directly by the gateway/UI, not the bridge) is asserted on top.
    #[test]
    fn host_catalog_covers_dispatch_prefixes() {
        let cat = host_catalog();
        for prefix in crate::tool_call::HOST_NATIVE_PREFIXES
            .iter()
            .chain(["system."].iter())
        {
            assert!(
                cat.iter().any(|t| t.tool.starts_with(prefix)),
                "host catalog has no entry for dispatched prefix `{prefix}`"
            );
        }
        for exact in crate::tool_call::HOST_NATIVE_EXACT {
            assert!(
                cat.iter().any(|t| &t.tool == exact),
                "host catalog has no entry for dispatched verb `{exact}`"
            );
        }
    }

    #[test]
    fn every_row_is_well_formed() {
        for t in host_catalog() {
            assert!(!t.tool.is_empty(), "empty tool name");
            assert!(
                !t.description.is_empty(),
                "tool {} has no description",
                t.tool
            );
            assert_eq!(t.source, "host");
            assert!(!t.group.is_empty(), "tool {} has no group", t.tool);
        }
    }

    /// The split into per-family files must not have duplicated a row (a copy/paste of a family slice
    /// into two `FAMILIES` entries would double every verb in the console menu).
    #[test]
    fn no_duplicate_rows() {
        let cat = host_catalog();
        for w in cat.windows(2) {
            assert_ne!(
                w[0].tool, w[1].tool,
                "duplicate catalog row `{}`",
                w[0].tool
            );
        }
    }
}
