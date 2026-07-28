//! The `dashboard.*` family — the grid-of-widgets surface verbs (dashboard scope).
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`.

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const DASHBOARD: &[HostTool] = &[
    // dashboard.* — the grid-of-widgets surface verbs (dashboard scope).
    HostTool {
        tool: "dashboard.get",
        group: "dashboard",
        description: "read one dashboard by id",
    },
    HostTool {
        tool: "dashboard.list",
        group: "dashboard",
        description: "list the dashboards visible to the caller",
    },
    HostTool {
        tool: "dashboard.save",
        group: "dashboard",
        description: "create or update a dashboard the caller owns",
    },
    HostTool {
        tool: "dashboard.save_any",
        group: "dashboard",
        description:
            "admin override: save any dashboard in the workspace, not just the caller's own",
    },
    HostTool {
        tool: "dashboard.delete",
        group: "dashboard",
        description: "delete a dashboard the caller owns",
    },
    HostTool {
        tool: "dashboard.delete_any",
        group: "dashboard",
        description:
            "admin override: delete any dashboard in the workspace, not just the caller's own",
    },
    HostTool {
        tool: "dashboard.share",
        group: "dashboard",
        description: "share a dashboard with another principal/team",
    },
    HostTool {
        tool: "dashboard.share_any",
        group: "dashboard",
        description:
            "admin override: share any dashboard in the workspace, not just the caller's own",
    },
    HostTool {
        tool: "dashboard.access_check",
        group: "dashboard",
        description:
            "read-only preflight: walk a dashboard's dependency closure and report, per dependency, whether a subject/team can render it (access-model scope)",
    },
    HostTool {
        tool: "dashboard.import",
        group: "dashboard",
        description:
            "import a Grafana dashboard JSON (preview returns a datasource-remap report; commit with mappings upserts a dashboard the caller owns)",
    },
    HostTool {
        tool: "dashboard.export",
        group: "dashboard",
        description: "export a dashboard the caller can read as Grafana JSON",
    },
    HostTool {
        tool: "dashboard.pin",
        group: "dashboard",
        description: "mint a cell from a render envelope and upsert it into a dashboard (owner-only update)",
    },
    HostTool {
        tool: "dashboard.share_closure",
        group: "dashboard",
        description: "plan (or, with dry_run false, apply) the shares a team needs to reach a dashboard's dependencies",
    },
];
