//! The `cache.*` family — the optional response cache's admin surface (response-cache scope).
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`.

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const CACHE: &[HostTool] = &[
    // cache.* — the optional response cache's admin surface (response-cache scope). Present in the
    // catalog whether or not the `page-cache` feature is compiled in (the family is dispatched either
    // way — feature-off it returns NotFound), so the coverage assertion holds in both builds.
    HostTool {
        tool: "cache.stats",
        group: "cache",
        description: "response-cache counters: hits/misses/evictions, entry count, weighted size, per-class",
    },
    HostTool {
        tool: "cache.purge",
        group: "cache",
        description: "drop this workspace's cached reads (a bounded generation bump — the stale-data escape hatch)",
    },
];
