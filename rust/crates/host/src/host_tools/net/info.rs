//! `host.net.info` — hostname and classified interface addresses.

use serde::Serialize;

use super::platform;

#[derive(Debug, Clone, Serialize)]
pub struct HostNetInfo {
    pub hostname: String,
    pub interfaces: Vec<HostNetInterface>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostNetInterface {
    pub name: String,
    pub addresses: Vec<HostNetAddress>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostNetAddress {
    pub ip: String,
    pub family: String,
    pub scope: String,
}

pub fn host_net_info() -> HostNetInfo {
    HostNetInfo {
        hostname: platform::hostname(),
        interfaces: platform::interfaces()
            .into_iter()
            .map(|iface| HostNetInterface {
                name: iface.name,
                addresses: iface
                    .addresses
                    .into_iter()
                    .map(|addr| HostNetAddress {
                        ip: addr.ip.to_string(),
                        family: if addr.ip.is_ipv4() { "ipv4" } else { "ipv6" }.to_string(),
                        scope: platform::classify_ip(addr.ip).to_string(),
                    })
                    .collect(),
            })
            .collect(),
    }
}
