//! The `viz.*` bridge (the ONE viz read) + the `query.*` saved-query family.
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`.

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const VIZ: &[HostTool] = &[
    // viz.query — the ONE viz bridge (widget-platform scope).
    HostTool {
        tool: "viz.query",
        group: "viz",
        description: "run a saved/inline query shaped for charts + tables (the one viz bridge)",
    },
    // viz.query_batch — the batch fan-in (dashboard-query-acceleration scope, slice 3). Rides the
    // SAME `mcp:viz.query:call` cap (a fan-in of the same read, no new privilege) — so the catalog's
    // per-tool `authorize_tool(principal, ws, gate_tool_for(name))` gate makes it visible to exactly
    // the callers who can run `viz.query`.
    HostTool {
        tool: "viz.query_batch",
        group: "viz",
        description: "resolve many panels in ONE call, concurrently (per-item partial failure)",
    },
    // query.* — saved queries (query-workbench scope).
    HostTool {
        tool: "query.run",
        group: "query",
        description: "run a saved query by id (with optional parameter overrides)",
    },
    HostTool {
        tool: "query.save",
        group: "query",
        description: "save a named query definition",
    },
    HostTool {
        tool: "query.compile",
        group: "query",
        description: "compile a query definition to its target SQL without running it",
    },
    HostTool {
        tool: "query.get",
        group: "query",
        description: "read one saved query definition",
    },
    HostTool {
        tool: "query.list",
        group: "query",
        description: "list the workspace's saved queries",
    },
    HostTool {
        tool: "query.delete",
        group: "query",
        description: "delete a saved query",
    },
];
