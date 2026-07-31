//! Advertise this node on the LAN — the responder half.
//!
//! **Opt-in and non-fatal.** A node that never calls this is invisible to mDNS, which is the
//! default posture. When it IS called and the network refuses (no multicast interface, a
//! locked-down container), the caller logs and continues serving: discovery failing must never
//! take down a node that is otherwise healthy. That is why this returns a `Result` the boot path
//! is expected to warn on rather than propagate.
//!
//! The advertisement is retracted when [`Advertisement`] drops — the same lifetime discipline as
//! `lb_bus::Presence`, so a clean shutdown removes the record instead of leaving a stale one for
//! the TTL. A crash leaves the record until it ages out; mDNS has no liveliness equivalent, which
//! is precisely why this crate is bootstrap-only and fleet-presence remains authoritative.

use lb_bus::NodeId;
use mdns_sd::{ServiceDaemon, ServiceInfo};

use crate::error::DiscoveryError;
use crate::identity::{NodeIdentity, TXT_MACHINE, TXT_NAME};
use crate::peer::{TXT_FLEET, TXT_NODE, TXT_VERSION};
use crate::service_type::ServiceType;

/// What this node publishes about itself. Reachability and identity only — see `peer.rs` and
/// `identity.rs` for why there is nowhere here to put a workspace.
#[derive(Debug, Clone)]
pub struct Advertisement {
    /// The service type to advertise under. Product-agnostic by default (`_lb._tcp`).
    pub service_type: ServiceType,
    /// Who this node is: the addressable id fleet-presence announces, plus the optional
    /// machine-derived id and the operator's human label ([`NodeIdentity`]).
    pub identity: NodeIdentity,
    /// The port peers should dial (typically the gateway's).
    pub port: u16,
    /// Version string peers can use for compatibility checks before dialing.
    pub version: Option<String>,
    /// Opaque operator grouping tag; not a security boundary.
    pub fleet: Option<String>,
}

impl Advertisement {
    /// A minimal advertisement: this node, this port, default service type.
    pub fn new(node: NodeId, port: u16) -> Self {
        Self::with_identity(NodeIdentity::new(node), port)
    }

    /// An advertisement for a fully-formed identity — the form an embedder that has a machine id
    /// and an operator-set name uses.
    pub fn with_identity(identity: NodeIdentity, port: u16) -> Self {
        Self {
            service_type: ServiceType::default(),
            identity,
            port,
            version: None,
            fleet: None,
        }
    }

    /// The addressable node id this advertisement publishes under.
    pub fn node(&self) -> &NodeId {
        self.identity.node()
    }
}

/// A live advertisement. While held, this node is discoverable; drop it to retract the record.
pub struct Advertised {
    daemon: ServiceDaemon,
    fullname: String,
}

impl Drop for Advertised {
    fn drop(&mut self) {
        // Best-effort: unregister, then shut the daemon down. Both return receivers we
        // deliberately do not await — `drop` cannot block on a channel, and a failure here only
        // means the record ages out on its TTL instead of vanishing immediately.
        let _ = self.daemon.unregister(&self.fullname);
        let _ = self.daemon.shutdown();
    }
}

impl Advertised {
    /// The registered instance name, e.g. `node:gw-01._lb._tcp.local.` — useful in logs and for
    /// correlating with `avahi-browse` output.
    pub fn fullname(&self) -> &str {
        &self.fullname
    }
}

/// Start advertising this node. Hold the returned [`Advertised`] for as long as the node serves.
///
/// The instance name is the node id, which makes the mDNS record self-describing under standard
/// tooling and means two nodes cannot collide unless their ids already collide — a condition
/// fleet-presence treats as a fault anyway.
pub fn advertise(ad: &Advertisement) -> Result<Advertised, DiscoveryError> {
    let daemon = ServiceDaemon::new().map_err(|e| DiscoveryError::Responder(e.to_string()))?;

    let mut properties = vec![
        (TXT_NODE.to_string(), ad.node().to_string()),
        // The human label always rides along: it is never absent (it defaults to the node id), and
        // an operator browsing with `avahi-browse` wants to read a name, not a uuid.
        (TXT_NAME.to_string(), ad.identity.name.clone()),
        // An empty string is a legal TXT value; only include the optional keys when set so a
        // browsing peer can distinguish "not advertised" from "advertised as empty".
    ];
    // Cleartext on the wire — the embedder is responsible for having passed a non-reversible form
    // (see `NodeIdentity::machine_id`). Omitted entirely when the embedder had no source for one.
    if let Some(m) = &ad.identity.machine_id {
        properties.push((TXT_MACHINE.to_string(), m.clone()));
    }
    if let Some(v) = &ad.version {
        properties.push((TXT_VERSION.to_string(), v.clone()));
    }
    if let Some(f) = &ad.fleet {
        properties.push((TXT_FLEET.to_string(), f.clone()));
    }

    // `enable_addr_auto` lets the daemon fill in (and keep updating) this host's addresses across
    // every multicast-capable interface. Hand-picking one is the classic multi-homed bug: a node
    // advertises the address of an interface the peer cannot route to.
    let hostname = format!("{}.local.", sanitize_hostname(ad.node().as_str()));
    let info = ServiceInfo::new(
        &ad.service_type.fqdn(),
        ad.node().as_str(),
        &hostname,
        "",
        ad.port,
        &properties[..],
    )
    .map_err(|e| DiscoveryError::Responder(e.to_string()))?
    .enable_addr_auto();

    let fullname = info.get_fullname().to_string();
    daemon
        .register(info)
        .map_err(|e| DiscoveryError::Responder(e.to_string()))?;

    tracing::info!(
        node = %ad.node(),
        service_type = ad.service_type.as_str(),
        port = ad.port,
        "advertising node on the local network via mDNS"
    );

    Ok(Advertised { daemon, fullname })
}

/// Make a node id usable as a DNS hostname label.
///
/// `NodeId` permits `:` (the platform's readable `node:gw-01` convention) but a DNS label does
/// not, so the host label — unlike the instance name, which is free-form UTF-8 per RFC 6763 —
/// needs the character folded away.
fn sanitize_hostname(node: &str) -> String {
    node.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_the_colon_convention_into_a_dns_safe_label() {
        // `node:gw-01` is a legal NodeId but an illegal DNS label; the `:` must not reach the wire.
        assert_eq!(sanitize_hostname("node:gw-01"), "node-gw-01");
    }

    #[test]
    fn leaves_an_already_safe_label_alone() {
        assert_eq!(sanitize_hostname("gw-01"), "gw-01");
    }
}
