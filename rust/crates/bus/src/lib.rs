//! The event bus — an embedded Zenoh peer (README §6.2). The host process *is* a bus peer;
//! no separate broker. Motion only (§3.3): the bus moves messages; state lives in the store.
//!
//! Every key is namespaced per workspace: a workspace-relative key `chan/general` is
//! published under `ws/{id}/chan/general`. The `ws/{id}/` prefix is applied here, never
//! written by callers — that keeps the workspace wall on the bus structural (§6.2, §7).
//!
//! Edge nodes run in peer mode and connect up to a router (cloud hub); the mode is config,
//! not code (symmetric nodes, §3.1). S1 runs a solo peer. Durable delivery is the outbox's
//! job (§6.10), not raw pub/sub — so S1 exposes only the peer lifecycle + key scoping; the
//! pub/sub verbs land with the messaging slice (S2).

mod await_subscriber;
mod key;
mod node_id;
mod peer;
mod presence;
mod publish;
mod query;
mod stats;
mod subscribe;

pub use await_subscriber::await_subscriber;
pub use key::ws_key;
pub use node_id::{NodeId, NodeIdError};
pub use peer::{Bus, BusError};
pub use presence::{declare_presence, watch_presence, Presence, PresenceWatch};
pub use publish::publish;
pub use query::{declare_queryable, query, Incoming, Responder};
pub use stats::{bus_stats, BusStats};
pub use subscribe::{subscribe, Subscription};
