//! What a discovery result actually is: an endpoint to dial and who claims to be there — nothing more.
//!
//! The shape of this struct IS the workspace-wall argument (lib docs). Every field here is
//! readable by anything on the LAN segment, so the type deliberately has no place to put a
//! workspace, a persona, a capability, or an extension list. If a future change wants to add one,
//! that is the signal it belongs on the fleet-presence roster — behind the bus, inside the wall —
//! not in an mDNS TXT record.
//!
//! `name` and `machine_id` (the identity trio's other two thirds — see `identity.rs`) pass that
//! bar and only that bar: they say *which* node answers, which is the same class of fact as `node`
//! and `hostname` that this record already carried. They grant nothing and prove nothing — like
//! every value here they are cleartext and forgeable, so they identify *accidents*, not
//! *adversaries*, and the dial that follows still authenticates.

use std::net::IpAddr;

use lb_bus::NodeId;

/// TXT keys carried on the wire. Kept short: a TXT record is size-constrained, and these are
/// parsed by peers that may be running a different version than the advertiser.
pub(crate) const TXT_NODE: &str = "node";
pub(crate) const TXT_VERSION: &str = "ver";
pub(crate) const TXT_FLEET: &str = "fleet";

/// An lb node seen on the local network.
///
/// Presence here means *reachable*, not *trusted* and not *joined*: the dial that follows still
/// authenticates, and the caps wall still gates every call. A peer appearing in a browse has
/// proven only that it is on the same wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredPeer {
    /// The advertiser's stable node id — the SAME identity fleet-presence announces on the bus,
    /// so a peer discovered here can be correlated with a roster entry once connected.
    pub node: NodeId,

    /// Addresses the responder published. More than one is normal (multi-homed host, v4 + v6);
    /// callers should try them in order rather than assuming the first works.
    pub addresses: Vec<IpAddr>,

    /// The advertised service port — the endpoint to dial.
    pub port: u16,

    /// The advertiser's operator-set human label (`NodeIdentity::name`). Present on any node
    /// running a version that advertises it; `None` from an older responder.
    ///
    /// **Display text, never an identifier** — do not address, route to, or key off it. Two peers
    /// may legitimately report the same name, and it is trivially forged on the wire like every
    /// other TXT value. `node` remains the only identity that means anything.
    pub name: Option<String>,

    /// The advertiser's opaque machine-derived id, when it published one.
    ///
    /// Lets an operator tell "this box was reinstalled" (same `machine_id`, new `node`) from "this
    /// is a different box". Never parsed or interpreted here, and **not** a trust signal: it is
    /// cleartext on the LAN and forgeable, so it diagnoses accidents, not adversaries — the same
    /// caveat `fleet` carries.
    pub machine_id: Option<String>,

    /// The advertiser's version string, for compatibility decisions before dialing.
    pub version: Option<String>,

    /// An opaque operator-set grouping tag. Lets one LAN host several unrelated fleets without
    /// them adopting each other. **Not a security boundary** — it is plaintext on the wire and
    /// trivially forged; it separates *accidents*, not *adversaries*.
    pub fleet: Option<String>,

    /// The responder's mDNS hostname, for logging and operator diagnosis.
    pub hostname: String,
}

impl DiscoveredPeer {
    /// The first address paired with the port, as a dialable `host:port`. Returns `None` for a
    /// record that resolved with no addresses (possible when a responder is shutting down).
    pub fn endpoint(&self) -> Option<String> {
        self.addresses.first().map(|ip| match ip {
            IpAddr::V4(v4) => format!("{v4}:{}", self.port),
            // A v6 literal must be bracketed or the `:port` is ambiguous with the address itself.
            IpAddr::V6(v6) => format!("[{v6}]:{}", self.port),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    fn peer(addresses: Vec<IpAddr>) -> DiscoveredPeer {
        DiscoveredPeer {
            node: NodeId::new("node:gw-01").unwrap(),
            addresses,
            port: 8099,
            name: None,
            machine_id: None,
            version: None,
            fleet: None,
            hostname: "gw-01.local.".into(),
        }
    }

    #[test]
    fn formats_a_v4_endpoint() {
        let p = peer(vec![IpAddr::V4(Ipv4Addr::new(192, 168, 1, 47))]);
        assert_eq!(p.endpoint().as_deref(), Some("192.168.1.47:8099"));
    }

    #[test]
    fn brackets_a_v6_endpoint() {
        // Unbracketed, `fe80::1:8099` cannot be parsed back — the port merges into the address.
        let p = peer(vec![IpAddr::V6(Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1))]);
        assert_eq!(p.endpoint().as_deref(), Some("[fe80::1]:8099"));
    }

    #[test]
    fn an_addressless_record_yields_no_endpoint() {
        assert_eq!(peer(vec![]).endpoint(), None);
    }
}
