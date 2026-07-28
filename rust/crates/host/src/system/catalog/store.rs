//! The `store.*` family — the parse-allowlisted SQL read surface, the write half, and the operational pair.
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`.

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const STORE: &[HostTool] = &[
    // store.* — the read-only, parse-allowlisted SQL surface a widget/page reads (store-query scope).
    HostTool {
        tool: "store.query",
        group: "store",
        description: "a bounded, workspace-walled read-only SELECT over the embedded store",
    },
    HostTool {
        tool: "store.schema",
        group: "store",
        description: "the store schema (tables + columns) for the visual query builder",
    },
    // store operational pair (online-compaction scope, issue #67).
    HostTool {
        tool: "store.status",
        group: "store",
        description: "commit-log size, segment count, and last-compaction outcome (observability)",
    },
    HostTool {
        tool: "store.compact",
        group: "store",
        description: "enqueue a commit-log compaction pass as a job (admin; whole-log I/O)",
    },
    // store.* write half (the read half is above).
    HostTool {
        tool: "store.write",
        group: "store",
        description: "write one record into the embedded store",
    },
    HostTool {
        tool: "store.delete",
        group: "store",
        description: "delete one record from the embedded store",
    },
];
