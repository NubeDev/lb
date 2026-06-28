//! `host.time.now` — current UTC/local time and local zone.

use chrono::{SecondsFormat, Utc};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct HostTimeNow {
    pub utc: String,
    pub local: String,
    pub zone: String,
    pub offset_seconds: i32,
}

pub fn host_time_now() -> HostTimeNow {
    let utc = Utc::now();
    let local = chrono::Local::now();
    HostTimeNow {
        utc: utc.to_rfc3339_opts(SecondsFormat::Secs, true),
        local: local.to_rfc3339_opts(SecondsFormat::Secs, false),
        zone: iana_time_zone::get_timezone().unwrap_or_else(|_| "Etc/UTC".to_string()),
        offset_seconds: local.offset().local_minus_utc(),
    }
}
