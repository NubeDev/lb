//! The `assets.*` family (docs, skills, binary assets) + the `docs.*` doc-derived operations.
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`.

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const ASSETS: &[HostTool] = &[
    // assets.* — docs, skills, and binary assets (assets scope).
    HostTool {
        tool: "assets.list_docs",
        group: "assets",
        description: "list the workspace's shared docs",
    },
    HostTool {
        tool: "assets.get_doc",
        group: "assets",
        description: "read one shared doc",
    },
    HostTool {
        tool: "assets.link_doc",
        group: "assets",
        description: "link a doc to another record",
    },
    HostTool {
        tool: "assets.delete_doc",
        group: "assets",
        description: "delete a shared doc",
    },
    HostTool {
        tool: "assets.list_assets",
        group: "assets",
        description: "list the workspace's binary assets",
    },
    HostTool {
        tool: "assets.get_asset",
        group: "assets",
        description: "read one binary asset's metadata",
    },
    HostTool {
        tool: "assets.delete_asset",
        group: "assets",
        description: "delete a binary asset",
    },
    HostTool {
        tool: "assets.backlinks",
        group: "assets",
        description: "the records linking to a doc/asset",
    },
    HostTool {
        tool: "assets.list_granted_skills",
        group: "assets",
        description: "list the skills granted to you",
    },
    HostTool {
        tool: "assets.load_skill",
        group: "assets",
        description: "load a granted skill's body (grant-gated)",
    },
    // docs.* — doc-derived operations (doc-extraction scope; embeddings scope adds search/reindex).
    HostTool {
        tool: "docs.extract",
        group: "docs",
        description: "derive markdown docs from binary media (PDF/XLSX/CSV/HTML/text)",
    },
];
