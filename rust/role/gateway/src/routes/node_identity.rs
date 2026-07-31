//! `GET /node` — the unauthenticated node-identity probe (node-identity scope).
//!
//! The public answer to "who and where is this node?", for a caller that has no session token: a
//! provisioning tool, an installer, a mobile app on the LAN, or a human with `curl`. It is the HTTP
//! twin of the mDNS advertisement — the same identity, reachable by anything that can already reach
//! the port, so a caller that found the node by dialing it gets the same facts a caller that found
//! it by browsing does.
//!
//! ```text
//! GET /node  →  200  {"node":"node:gw-01","name":"front office","machine_id":"…",
//!                     "version":"…","gateway":{"port":8099,"addresses":["192.168.1.40"]}}
//!            →  404  (no identity configured — see below)
//! ```
//!
//! **Why `/node`, not `/identity`.** `/admin/identities` is the *people* directory (global-identity
//! scope). Node identity and human identity are unrelated, and giving them adjacent names would be
//! a standing invitation to confuse them in a router, a client, and a log.
//!
//! # Why it can 404
//!
//! The route is only meaningful when the embedder installed a [`NodeIdentity`] on
//! `BootConfig::identity`. Without one the node is running the fresh-per-process random id `boot*`
//! mints — publishing that would be actively misleading: a caller would cache an "identity" that
//! changes on the next restart. `404` says "this node has no durable identity to report", which is
//! the honest answer and distinguishable from a node that is down (connection refused) or degraded
//! (`/health` 503).
//!
//! # The wall (rule 6 — the same one `lb-discovery` draws)
//!
//! This is served **outside the auth wall**, the same posture as `GET /health` and `POST /login`,
//! so it obeys the identical rule the mDNS record does: **reachability and identity only**. No
//! workspace, no persona, no capability, no extension list, no member — nothing that is inside a
//! wall may appear here. Everything in the body is already broadcast in cleartext over mDNS by a
//! discoverable node, so this route reveals nothing that discovery does not.
//!
//! Concretely, what a caller learns is: this port belongs to a node with this id and this name.
//! Learning that grants no access — the caps wall and workspace isolation gate every byte after,
//! exactly as they do for a peer that arrived via discovery. **Addressing is not authorization.**
//!
//! `machine_id` is the one field with a specific caution, and it is the embedder's to honour: a raw
//! OS machine-id must not be exposed on an untrusted network. The gateway cannot tell where the
//! string came from, so it publishes what it was given — `lb_discovery::NodeIdentity::machine_id`
//! states the requirement at the field an embedder fills.
//!
//! **Reads in-memory state only** — no store query, no disk I/O, no network call, same as `/health`.
//! The identity is a boot-time constant; a probe can never block on a dependency.

use std::net::IpAddr;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;

use crate::state::Gateway;

/// The `gateway` sub-object: where to reach this node's HTTP surface.
///
/// `addresses` is a **list**, and callers must treat it as one. A multi-homed host (several NICs, a
/// VPN, v4 + v6) legitimately answers on more than one address, and there is no basis for the node
/// to pick the caller's best route on its behalf — the classic multi-homed bug is a node confidently
/// publishing the address of an interface the caller cannot reach. This is the same reason
/// `DiscoveredPeer::addresses` is a `Vec`. Try them in order.
///
/// The list is **empty** when the node binds a wildcard (`0.0.0.0`) and interface enumeration was
/// not supplied by the embedder — an honest empty beats an unroutable `0.0.0.0` that a client would
/// dutifully try. A caller that reached this route already knows one working address: its own
/// connection's.
#[derive(Debug, Serialize)]
pub struct GatewayEndpoint {
    /// The port the gateway serves on — the one to dial.
    pub port: u16,
    /// Candidate addresses this node believes it answers on. May be empty; try in order.
    pub addresses: Vec<IpAddr>,
}

/// The `GET /node` body. Identity + reachability, and deliberately nothing else.
#[derive(Debug, Serialize)]
pub struct NodeBody {
    /// The addressable node id — the same one the bus roster and the mDNS record carry.
    pub node: String,
    /// The operator-set human label. Display text, never an identifier.
    pub name: String,
    /// The opaque machine-derived id, omitted entirely when the embedder supplied none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub machine_id: Option<String>,
    /// The gateway build version — the same value `/health` reports.
    pub version: &'static str,
    /// Where to reach this node's HTTP surface.
    pub gateway: GatewayEndpoint,
}

/// `GET /node` — unauthenticated, in-memory, one route. `200` with the node's identity when the
/// embedder configured one; `404` when it did not (see the module docs for why that is not an
/// error condition). Never touches the store.
pub async fn node_identity(State(gw): State<Gateway>) -> Result<Json<NodeBody>, StatusCode> {
    // No configured identity ⇒ the node has only a throwaway per-boot id, which must not be
    // published as though it were durable.
    let identity = gw.identity.as_ref().as_ref().ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(NodeBody {
        node: identity.node().to_string(),
        name: identity.name.clone(),
        machine_id: identity.machine_id.clone(),
        version: crate::routes::VERSION,
        gateway: GatewayEndpoint {
            port: gw.bound_port.unwrap_or_default(),
            addresses: gw.bound_addresses.to_vec(),
        },
    }))
}
