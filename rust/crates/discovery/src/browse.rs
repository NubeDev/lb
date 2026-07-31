//! Discover lb nodes on the LAN — the browser half.
//!
//! Mirrors `lb_bus::watch_presence`'s shape (a handle you `recv()` in a loop) so the two
//! discovery layers read the same way at the call site, even though only the bus one is
//! authoritative for workspace presence.
//!
//! `mdns-sd` hands back a `flume` receiver; we bridge it onto tokio via `recv_async` so a caller
//! can select over it alongside the rest of a node's async work without a blocking thread.

use mdns_sd::{Receiver, ServiceDaemon, ServiceEvent};

use lb_bus::NodeId;

use crate::error::DiscoveryError;
use crate::identity::{TXT_MACHINE, TXT_NAME};
use crate::peer::{DiscoveredPeer, TXT_FLEET, TXT_NODE, TXT_VERSION};
use crate::service_type::ServiceType;

/// A peer appearing or disappearing on the LAN.
#[derive(Debug, Clone)]
pub enum Discovered {
    /// A node resolved with a usable address set — ready to dial.
    Found(DiscoveredPeer),
    /// A node's record went away (clean shutdown, or its TTL expired).
    ///
    /// **Not an offline signal.** mDNS removal is slow and unreliable compared to a Zenoh
    /// liveliness retraction: a crashed node lingers for its TTL. Treat this as "stop offering
    /// this endpoint", never as "this node is down" — that judgement belongs to fleet-presence.
    Removed { fullname: String },
}

/// A live browse over the LAN. Hold it and call [`Discovered`] events off `recv`; drop it to stop.
pub struct Browse {
    daemon: ServiceDaemon,
    // `mdns_sd::Receiver` is the crate's own re-export of its channel type — named through
    // `mdns-sd` rather than by adding a direct `flume` dep we would have to keep pinned in
    // lockstep with theirs.
    events: Receiver<ServiceEvent>,
    service_type: String,
}

impl Drop for Browse {
    fn drop(&mut self) {
        let _ = self.daemon.stop_browse(&self.service_type);
        let _ = self.daemon.shutdown();
    }
}

impl Browse {
    /// Await the next discovery event.
    ///
    /// Records that do not parse as an lb node are skipped silently rather than surfaced: a
    /// browse sees every responder under the type, including other software and older/newer node
    /// versions, and a peer we cannot understand is not an error the caller can act on.
    pub async fn recv(&self) -> Option<Discovered> {
        loop {
            let event = self.events.recv_async().await.ok()?;
            match event {
                ServiceEvent::ServiceResolved(resolved) => {
                    if let Some(peer) = to_peer(&resolved) {
                        return Some(Discovered::Found(peer));
                    }
                    tracing::debug!(
                        fullname = resolved.get_fullname(),
                        "skipping an mDNS record that is not a recognisable lb node"
                    );
                }
                ServiceEvent::ServiceRemoved(_ty, fullname) => {
                    return Some(Discovered::Removed { fullname })
                }
                // Search lifecycle events carry no peer information.
                ServiceEvent::SearchStarted(_)
                | ServiceEvent::ServiceFound(_, _)
                | ServiceEvent::SearchStopped(_) => continue,
                _ => continue,
            }
        }
    }
}

/// Start browsing for nodes of `service_type`.
pub fn browse(service_type: &ServiceType) -> Result<Browse, DiscoveryError> {
    let daemon = ServiceDaemon::new().map_err(|e| DiscoveryError::Responder(e.to_string()))?;
    let fqdn = service_type.fqdn();
    let events = daemon
        .browse(&fqdn)
        .map_err(|e| DiscoveryError::Responder(e.to_string()))?;

    tracing::info!(
        service_type = service_type.as_str(),
        "browsing the local network for lb nodes"
    );

    Ok(Browse {
        daemon,
        events,
        service_type: fqdn,
    })
}

/// Parse a resolved mDNS record into a peer, or `None` if it is not one of ours.
///
/// The node id is **required and validated** — an id arriving over the wire goes through
/// `NodeId::new` exactly like one from config, so a malicious responder cannot smuggle a
/// key-expression wildcard (`gw-*`) into a peer that later reaches a bus key. This is the same
/// argument `NodeId`'s serde impl makes, applied at the other untrusted boundary.
fn to_peer(resolved: &mdns_sd::ResolvedService) -> Option<DiscoveredPeer> {
    let node = NodeId::new(resolved.get_property_val_str(TXT_NODE)?).ok()?;

    Some(DiscoveredPeer {
        node,
        // `ScopedIp` carries an optional interface scope; the bare address is what a caller dials.
        addresses: resolved
            .get_addresses()
            .iter()
            .map(|s| s.to_ip_addr())
            .collect(),
        port: resolved.get_port(),
        // Identity extras: absent from an older responder that predates them, which is why both
        // are `Option` and neither is required to build a usable peer.
        name: resolved.get_property_val_str(TXT_NAME).map(str::to_string),
        machine_id: resolved
            .get_property_val_str(TXT_MACHINE)
            .map(str::to_string),
        version: resolved
            .get_property_val_str(TXT_VERSION)
            .map(str::to_string),
        fleet: resolved.get_property_val_str(TXT_FLEET).map(str::to_string),
        hostname: resolved.get_hostname().to_string(),
    })
}
