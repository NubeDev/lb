//! The `tags.*` family — the typed annotation + relationship graph.
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`.
//!
//! These four verbs had a full host service and per-verb caps since the tags scope and **no
//! dispatcher entry**, so nothing could reach them over MCP — and no catalog rows either, because a
//! family that cannot be dispatched has nothing to advertise. The case plane opened the door (it is
//! what makes a `human`-sourced tag edge writable at all, and therefore what makes the
//! `Human > Producer` precedence rule reachable in production), so the rows land with it.
//!
//! `tags.of` advertises under the **`tags.find` read cap** it actually gates on (`tool_gate.rs` and
//! `tags/authorize.rs` both alias it): the catalog's cardinal rule is "advertise a tool only if the
//! call would allow it, never hide one that would pass", and that holds only if the catalog asks the
//! same question the gate does.

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const TAGS: &[HostTool] = &[
    HostTool {
        tool: "tags.add",
        group: "tags",
        description: "apply a tag to an entity with provenance (human/inferred/producer/system)",
    },
    HostTool {
        tool: "tags.remove",
        group: "tags",
        description: "drop an entity's edges for a tag key (and one value, if given)",
    },
    HostTool {
        tool: "tags.of",
        group: "tags",
        description: "every tag applied to one entity, with provenance",
    },
    HostTool {
        tool: "tags.find",
        group: "tags",
        description: "the entities matching all the given facets (exact / key-only)",
    },
];
