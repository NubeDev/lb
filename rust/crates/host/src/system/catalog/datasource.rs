//! The `datasource.*` / `federation.*` families — the external-datasource surface (datasources scope).
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`.

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const DATASOURCE: &[HostTool] = &[
    // datasource.* / federation.* — the external-datasource surface (datasources scope).
    HostTool {
        tool: "datasource.list",
        group: "datasource",
        description: "list the workspace's registered external datasources",
    },
    HostTool {
        tool: "datasource.test",
        group: "datasource",
        description: "test connectivity of a registered (or candidate) datasource",
    },
    HostTool {
        tool: "datasource.add",
        group: "datasource",
        description: "register an external datasource (DSN sealed into lb-secrets)",
    },
    HostTool {
        tool: "datasource.remove",
        group: "datasource",
        description: "remove a registered datasource",
    },
    HostTool {
        tool: "federation.query",
        group: "federation",
        description: "run SQL against one registered external datasource",
    },
    HostTool {
        tool: "federation.schema",
        group: "federation",
        description: "the tables + columns of one registered datasource",
    },
    HostTool {
        tool: "federation.mirror",
        group: "federation",
        description: "mirror an external query's rows into the embedded store",
    },
    HostTool {
        tool: "federation.write",
        group: "federation",
        description: "write rows to a registered datasource (bounded INSERT/UPSERT)",
    },
    HostTool {
        tool: "federation.delete",
        group: "federation",
        description: "delete rows from a registered datasource (bounded, structured key match)",
    },
    HostTool {
        tool: "federation.migrate",
        group: "federation",
        description: "plan/apply a designed schema to a datasource (additive DDL, dry-run default)",
    },
    HostTool {
        tool: "federation.export",
        group: "federation",
        description: "export platform series data to an external datasource (durable job)",
    },
    HostTool {
        tool: "dbschema.save",
        group: "dbschema",
        description: "save a designed schema record (tables/columns/PK/FK + layout)",
    },
    HostTool {
        tool: "dbschema.get",
        group: "dbschema",
        description: "read one designed schema record (full, layout included)",
    },
    HostTool {
        tool: "dbschema.list",
        group: "dbschema",
        description: "list the workspace's designed schema records (name + table count)",
    },
    HostTool {
        tool: "dbschema.delete",
        group: "dbschema",
        description: "remove a designed schema record (tombstones — never touches a live DB)",
    },
    // federation.sample — the AI-context snapshot verb, dispatched since the datasource-samples scope
    // but uncataloged until now (it rides `mcp:federation.query:call`, so it was reachable but hidden).
    HostTool {
        tool: "federation.sample",
        group: "federation",
        description: "a bounded row snapshot of a source's tables for AI context (rides the federation.query cap)",
    },
];
