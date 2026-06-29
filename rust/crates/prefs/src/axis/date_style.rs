//! `date_style` axis — how a calendar date is ordered (prefs scope, closed set). Independent of
//! language: Spanish text with USA date order must be expressible (the decouple-the-axes goal).

use serde::{Deserialize, Serialize};

/// Date field order. `eu` = `DD/MM/YYYY`, `iso` = `YYYY-MM-DD`, `usa` = `MM/DD/YYYY`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DateStyle {
    Eu,
    Iso,
    Usa,
}

impl DateStyle {
    pub fn as_str(&self) -> &'static str {
        match self {
            DateStyle::Eu => "eu",
            DateStyle::Iso => "iso",
            DateStyle::Usa => "usa",
        }
    }
    pub const ALL: [DateStyle; 3] = [DateStyle::Eu, DateStyle::Iso, DateStyle::Usa];
}
