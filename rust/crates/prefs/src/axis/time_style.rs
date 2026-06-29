//! `time_style` axis — 12-hour vs 24-hour clock (prefs scope, closed set). Region-seeded but
//! independently settable.

use serde::{Deserialize, Serialize};

/// Clock convention. `h12` = `2:30 PM`, `h24` = `14:30`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeStyle {
    H12,
    H24,
}

impl TimeStyle {
    pub fn as_str(&self) -> &'static str {
        match self {
            TimeStyle::H12 => "h12",
            TimeStyle::H24 => "h24",
        }
    }
    pub const ALL: [TimeStyle; 2] = [TimeStyle::H12, TimeStyle::H24];
}
