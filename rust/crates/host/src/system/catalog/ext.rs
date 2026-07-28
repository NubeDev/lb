//! The `ext.*` lifecycle verbs + the `pack.*` domain-pack family. Core knows no extension or pack by name (rule 10).
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`.

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const EXT: &[HostTool] = &[
    // ext.* — the ext-lifecycle family (ext-store-nodes scope): reachable over the one MCP bridge
    // (not only the gateway REST route) so a flow's `ext-list` node and any MCP client can call it.
    HostTool {
        tool: "ext.list",
        group: "ext",
        description: "list the workspace's installed extensions with live state (id, version, \
                      tier, enabled, running, health)",
    },
    HostTool {
        tool: "ext.enable",
        group: "ext",
        description: "enable an installed extension (durable intent the reconciler honors)",
    },
    HostTool {
        tool: "ext.disable",
        group: "ext",
        description: "disable an installed extension without evicting its binary",
    },
    HostTool {
        tool: "ext.uninstall",
        group: "ext",
        description: "uninstall an extension and evict its cached binary",
    },
    // pack.* — the domain-pack verb family (packs scope): one declaration turns a blank workspace
    // into a product. Core knows no pack by name (rule 10).
    HostTool {
        tool: "pack.validate",
        group: "pack",
        description: "parse a pack bundle and return its object plan, checksums, and lint findings",
    },
    HostTool {
        tool: "pack.apply",
        group: "pack",
        description: "apply a pack bundle to this workspace (idempotent; writes a receipt)",
    },
    HostTool {
        tool: "pack.list",
        group: "pack",
        description: "list the packs applied in this workspace",
    },
    HostTool {
        tool: "pack.get",
        group: "pack",
        description: "read one pack's apply receipt (manifest as applied + per-object outcomes)",
    },
];
