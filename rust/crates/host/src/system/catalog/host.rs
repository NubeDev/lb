//! The `host.*` family — cross-platform node introspection (host-tools scope).
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`.

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const HOST: &[HostTool] = &[
    // host.* — cross-platform node introspection (host-tools scope).
    HostTool {
        tool: "host.net.info",
        group: "host",
        description: "the node's hostname + network interfaces and their addresses",
    },
    HostTool {
        tool: "host.net.reach",
        group: "host",
        description: "test TCP reachability of a host:port from the node, with a timeout",
    },
    HostTool {
        tool: "host.time.now",
        group: "host",
        description: "the node's current UTC + local time, zone, and offset",
    },
    HostTool {
        tool: "host.time.zones",
        group: "host",
        description: "the time zones the node knows about",
    },
    HostTool {
        tool: "host.fs.stat",
        group: "host",
        description: "metadata for one path (exists, kind, size, mtime, permissions)",
    },
    HostTool {
        tool: "host.fs.list",
        group: "host",
        description: "a bounded directory listing with per-entry metadata; optional name/extensions/include_hidden filters",
    },
    HostTool {
        tool: "host.fs.home",
        group: "host",
        description: "the node's home directory (a stable anchor to browse from)",
    },
];
