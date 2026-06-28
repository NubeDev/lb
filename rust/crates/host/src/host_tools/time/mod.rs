//! Host time facts.

mod now;
mod zones;

pub use now::{host_time_now, HostTimeNow};
pub use zones::{host_time_zones, HostTimeZones};
