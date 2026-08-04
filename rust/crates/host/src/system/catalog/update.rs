//! The `update.*` family — the mediated surface for a node that can replace itself (node-update
//! scope). Present on every node; on one whose embedder configured no provider, `update.status`
//! answers `{"supported": false}` and the rest are `Unsupported` — advertised either way, because
//! "can this node update itself?" is a question the console must be able to ask.
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`.

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const UPDATE: &[HostTool] = &[
    HostTool {
        tool: "update.status",
        group: "update",
        description: "running version, backend, in-flight tx, last outcome, signing-key durability, credential state",
    },
    HostTool {
        tool: "update.check",
        group: "update",
        description: "the versions this node's update backend can reach, in the backend's own order",
    },
    HostTool {
        tool: "update.apply",
        group: "update",
        description: "accept an update to a version (admin; returns a tx, never a verdict)",
    },
    HostTool {
        tool: "update.rollback",
        group: "update",
        description: "accept a rollback to the backend's previous good state (admin; returns a tx)",
    },
    HostTool {
        tool: "update.history",
        group: "update",
        description: "the update backend's own journal, newest first",
    },
    HostTool {
        tool: "update.credential.status",
        group: "update",
        description: "whether this node is enrolled, from where, and the credential's fingerprint",
    },
    HostTool {
        tool: "update.credential.set",
        group: "update",
        description: "verify a credential against the backend, then seal it host-owned (admin)",
    },
    HostTool {
        tool: "update.credential.claim",
        group: "update",
        description: "run the backend's own enrolment handshake and seal the result host-owned (admin)",
    },
];
