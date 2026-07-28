//! The `template.*` family — the durable scripted-view snippets the widget builder persists.
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`.

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const TEMPLATE: &[HostTool] = &[
    // template.* — the durable scripted-view snippets the widget builder persists (widget-builder scope).
    HostTool {
        tool: "template.get",
        group: "template",
        description: "read one render template (Plot/D3/JSX snippet) by id",
    },
    HostTool {
        tool: "template.list",
        group: "template",
        description: "list the render templates visible to the caller",
    },
    HostTool {
        tool: "template.save",
        group: "template",
        description: "create or update a render template the caller authors",
    },
    HostTool {
        tool: "template.delete",
        group: "template",
        description: "delete a render template the caller authors",
    },
];
