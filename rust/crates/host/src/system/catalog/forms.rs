//! The `forms.*` family — the host's form asset (forms scope): a workspace-namespaced record holding
//! an opaque typed definition, mirroring `dashboard.*` simplified (no visibility tier).
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`.

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const FORMS: &[HostTool] = &[
    // forms.* — the form asset (forms scope). Dispatched by `tool_call.rs` under the `forms.` prefix
    // like every host-native family; the rows were missing, so the family was invisible to the console
    // and to the agent's `tools.catalog`-derived menu.
    HostTool {
        tool: "forms.get",
        group: "forms",
        description: "read one form by id (a tombstoned form reads as not-found)",
    },
    HostTool {
        tool: "forms.list",
        group: "forms",
        description: "list the forms in the workspace as summaries (no definition body)",
    },
    HostTool {
        tool: "forms.save",
        group: "forms",
        description: "create or update a form (idempotent upsert; update is owner-only)",
    },
    HostTool {
        tool: "forms.delete",
        group: "forms",
        description: "delete a form (idempotent tombstone; owner-only unless delete_any is held)",
    },
];
