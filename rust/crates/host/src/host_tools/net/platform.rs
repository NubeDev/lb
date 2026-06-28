//! Cross-platform network interface enumeration.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use if_addrs::{get_if_addrs, IfAddr};

#[derive(Debug, Clone)]
pub struct PlatformAddress {
    pub ip: IpAddr,
}

#[derive(Debug, Clone)]
pub struct PlatformInterface {
    pub name: String,
    pub addresses: Vec<PlatformAddress>,
}

pub fn hostname() -> String {
    hostname::get()
        .ok()
        .map(|s| s.to_string_lossy().trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

pub fn interfaces() -> Vec<PlatformInterface> {
    let mut out = Vec::new();
    for iface in get_if_addrs().unwrap_or_default() {
        let ip = match iface.addr {
            IfAddr::V4(v4) => IpAddr::V4(v4.ip),
            IfAddr::V6(v6) => IpAddr::V6(v6.ip),
        };
        if let Some(existing) = out
            .iter_mut()
            .find(|item: &&mut PlatformInterface| item.name == iface.name)
        {
            existing.addresses.push(PlatformAddress { ip });
        } else {
            out.push(PlatformInterface {
                name: iface.name,
                addresses: vec![PlatformAddress { ip }],
            });
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    for iface in &mut out {
        iface
            .addresses
            .sort_by(|a, b| a.ip.to_string().cmp(&b.ip.to_string()));
    }
    out
}

pub fn classify_ip(ip: IpAddr) -> &'static str {
    if ip.is_loopback() {
        "loopback"
    } else if is_private(ip) {
        "private"
    } else {
        "public"
    }
}

fn is_private(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => ipv4_private(ip),
        IpAddr::V6(ip) => ipv6_private(ip),
    }
}

fn ipv4_private(ip: Ipv4Addr) -> bool {
    ip.is_private() || ip.is_link_local() || ip.is_unspecified()
}

fn ipv6_private(ip: Ipv6Addr) -> bool {
    let first = ip.octets()[0];
    ip.is_unspecified() || ip.is_unicast_link_local() || (first & 0xfe) == 0xfc
}
