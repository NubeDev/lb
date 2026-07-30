//! LAN discovery over mDNS/DNS-SD — how a node finds a peer's **endpoint** before it has a bus.
//!
//! # Why this exists alongside fleet-presence
//!
//! These are two layers, not two answers to one question, and the difference is what a node
//! already has when it asks:
//!
//! - **fleet-presence** (`ws/{id}/nodes/{node_id}` Zenoh liveliness) answers *"which nodes are in
//!   THIS WORKSPACE's roster right now"*. It is authoritative for workspace presence — but it can
//!   only answer once a Zenoh session exists. It presumes connectivity.
//! - **this crate** answers *"what lb nodes are on this wire at all"*, with no bus, no session and
//!   no prior configuration. It is the **bootstrap** step that produces an endpoint to dial.
//!
//! The handoff is one-directional and ends here: mDNS yields an endpoint → the node dials it →
//! Zenoh connects → the liveliness roster takes over. Nothing in this crate is authoritative for
//! presence, and nothing here should grow a roster API; that would fork fleet-presence's job.
//!
//! Zenoh's own multicast scouting already covers the easy case (two peers, same subnet, multicast
//! unfiltered). This crate earns its place where scouting does not reach: a node handed no
//! endpoint on a network where multicast scouting is filtered but mDNS (a specific, commonly
//! permitted multicast group) survives, and the operator-facing case where `avahi-browse -rt` /
//! `dns-sd -B` must be able to see the fleet with standard tooling.
//!
//! # What this does NOT advertise (README §3 rule 6 — the workspace wall)
//!
//! An mDNS record is readable by anything on the LAN segment; it is outside every wall the
//! platform has. So the advertisement carries **reachability only** — node id, endpoint, version,
//! and an opaque operator-set fleet tag. It carries **no workspace id, no persona, no roster, no
//! capability, and no extension list**. Discovering a node tells a peer where to knock, never what
//! is inside: the caps wall and workspace isolation gate every byte after the dial, unchanged.
//! This is the same line routed-node-dispatch draws — addressing is not authorization.
//!
//! Because of that, [`ServiceType`] is **operator-supplied and product-agnostic** (default
//! `_lb._tcp`). A product host (`NubeIO/rubix-ai`) sets its own; no core crate names a product
//! (rule 10). Advertising is **opt-in** — a node that never calls [`advertise`] is invisible here.

mod advertise;
mod browse;
mod error;
mod peer;
mod service_type;

pub use advertise::{advertise, Advertised, Advertisement};
pub use browse::{browse, Browse, Discovered};
pub use error::DiscoveryError;
pub use peer::DiscoveredPeer;
pub use service_type::{ServiceType, DEFAULT_SERVICE_TYPE};
