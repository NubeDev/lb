//! Built-in `host.*` MCP node-introspection tools.

mod fs;
mod net;
mod time;
mod tool;

pub use fs::{host_fs_list, host_fs_stat, HostFsEntry, HostFsList, HostFsStat, HOST_FS_LIST_LIMIT};
pub use net::{
    host_net_info, host_net_reach, HostNetAddress, HostNetInfo, HostNetInterface, HostNetReach,
    HOST_NET_REACH_DEFAULT_TIMEOUT_MS, HOST_NET_REACH_MAX_TIMEOUT_MS,
};
pub use time::{host_time_now, host_time_zones, HostTimeNow, HostTimeZones};
pub use tool::call_host_tool;
