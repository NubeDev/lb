//! The `layout.*` family — the member-owned per-surface layout record (data-studio v2 scope).
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`.

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const LAYOUT: &[HostTool] = &[
    // layout.* — the member-owned per-surface layout record (data-studio v2 scope).
    HostTool {
        tool: "layout.get",
        group: "layout",
        description: "read your saved layout for a surface",
    },
    HostTool {
        tool: "layout.set",
        group: "layout",
        description: "save your layout for a surface",
    },
];
