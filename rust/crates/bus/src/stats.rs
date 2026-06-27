//! Live transport facts for the Zenoh peer session — the *real* liveness a system map needs, not
//! "the handle exists". One responsibility: read what `zenoh::Session::info()` already knows (this
//! node's id, and the ids of the peers/routers it is actually connected to on the mesh) and roll it
//! into a plain struct. Motion only (§3.3): this is the bus reporting on itself, no state written.
//!
//! `peers_zid()`/`routers_zid()` enumerate the transports currently established — so a `peer_count`
//! of `0` on a solo node is the honest truth (nothing else is on the mesh), and `N>0` is a real
//! connection count, not a guess. The reads are local session bookkeeping (no network round-trip).

use crate::peer::Bus;

/// A snapshot of the peer session's live connectivity. Cheap to gather (local session state).
#[derive(Debug, Clone)]
pub struct BusStats {
    /// This node's Zenoh id (the stable mesh identity), as a short hex string.
    pub zid: String,
    /// How many peer nodes this session is currently connected to (established transports).
    pub peer_count: usize,
    /// How many routers this session is currently connected to (0 on a pure peer-to-peer mesh).
    pub router_count: usize,
    /// The actual zids of the connected peers (the detail behind `peer_count` — for the system-map
    /// subsystem detail view, which lists *who* is on the mesh, not just how many).
    pub peer_zids: Vec<String>,
    /// The actual zids of the connected routers (the detail behind `router_count`).
    pub router_zids: Vec<String>,
}

/// Read the live transport stats from the Zenoh session. Enumerates the established peer/router
/// transports — a real count *and* the actual connected zids, gathered from local session
/// bookkeeping (no round-trip).
pub async fn bus_stats(bus: &Bus) -> BusStats {
    let info = bus.session().info();
    let zid = bus.session().zid().to_string();
    let peer_zids: Vec<String> = info.peers_zid().await.map(|z| z.to_string()).collect();
    let router_zids: Vec<String> = info.routers_zid().await.map(|z| z.to_string()).collect();
    BusStats {
        zid,
        peer_count: peer_zids.len(),
        router_count: router_zids.len(),
        peer_zids,
        router_zids,
    }
}
