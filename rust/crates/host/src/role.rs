//! Node role — **config, not a code branch** (README §3.1, §5). The same binary, built from
//! the same crates, plays edge / hub / solo by *which role it is configured as*. Core crates
//! never read this; only the wiring layers (the `node` binary, the role crates, and the host's
//! own assembly) consult it to decide *what to mount* — e.g. whether to run the sync relay or
//! expose the SSE gateway. There is no `if cloud { … }` inside a capability/store/bus path.
//!
//! Why an enum and not a `bool is_cloud`: roles are not a binary, and naming them keeps the
//! deployment intent explicit (§5 names exactly these three). Adding a role is a deliberate
//! config change, the same discipline as adding a capability surface.

/// Which role(s) a node plays. Selected by config at boot; the data authority and sync
/// direction follow from it (README §6.8) but the *code* is identical across roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Role {
    /// A user-device node: its own authority for node-local data, holds a read-cache of shared
    /// data, syncs edge→hub through the outbox. Works fully offline.
    Edge,
    /// The central hub: authority for shared workspace/identity data; the merge target the
    /// edges sync up to. (Runs a Zenoh router in a real deployment — endpoint config, not code.)
    Hub,
    /// An edge with no hub: its own authority for everything. The N=1 case (the S0–S2 posture).
    #[default]
    Solo,
}

impl Role {
    /// Is this node the **authority** for shared workspace data? The hub is; an edge holds a
    /// read-cache and queues writes up (README §6.8). A solo node is its own authority.
    ///
    /// This is the *only* role-derived policy the sync layer needs, and it is data authority —
    /// exactly the axis §3.1 permits roles to differ on, never a code path.
    pub fn is_shared_authority(self) -> bool {
        matches!(self, Role::Hub | Role::Solo)
    }
}
