//! `host.time.zones` — IANA timezone identifiers.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct HostTimeZones {
    pub zones: Vec<String>,
    pub count: usize,
}

pub fn host_time_zones() -> HostTimeZones {
    let zones: Vec<String> = chrono_tz::TZ_VARIANTS
        .iter()
        .map(|zone| zone.name().to_string())
        .collect();
    HostTimeZones {
        count: zones.len(),
        zones,
    }
}
