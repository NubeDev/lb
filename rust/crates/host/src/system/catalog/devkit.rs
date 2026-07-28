//! The `devkit.*` family — the in-app extension scaffolding/build toolkit (devkit scope).
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`.

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const DEVKIT: &[HostTool] = &[
    // devkit.* — the in-app extension scaffolding/build toolkit (devkit scope).
    HostTool {
        tool: "devkit.templates",
        group: "devkit",
        description: "list the extension scaffold templates available in Studio",
    },
    HostTool {
        tool: "devkit.root",
        group: "devkit",
        description: "the absolute devkit root directory the folder picker browses from",
    },
    HostTool {
        tool: "devkit.scaffold",
        group: "devkit",
        description: "scaffold a new extension from a template",
    },
    HostTool {
        tool: "devkit.write_file",
        group: "devkit",
        description: "write or replace a source file inside a scaffolded extension dir",
    },
    HostTool {
        tool: "devkit.inspect",
        group: "devkit",
        description: "inspect an extension's manifest + build inputs",
    },
    HostTool {
        tool: "devkit.build",
        group: "devkit",
        description: "build an extension's native sidecar + federated UI bundle",
    },
];
