//! Host networking facts.

mod info;
mod platform;
mod reach;

pub use info::{host_net_info, HostNetAddress, HostNetInfo, HostNetInterface};
pub use reach::{
    host_net_reach, HostNetReach, HOST_NET_REACH_DEFAULT_TIMEOUT_MS, HOST_NET_REACH_MAX_TIMEOUT_MS,
};
