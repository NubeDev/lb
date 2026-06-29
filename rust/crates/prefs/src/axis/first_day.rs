//! `first_day_of_week` axis — which weekday a calendar/week-picker starts on (prefs scope). A small
//! closed enum; region-seeded (US → Sunday, most of EU → Monday) but independently settable.

use serde::{Deserialize, Serialize};

/// The first day of the week. Closed to the two conventions the seed locales need; extend
/// deliberately (e.g. `saturday` for some MENA locales) when a catalog for that region lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirstDay {
    Monday,
    Sunday,
}

impl FirstDay {
    pub fn as_str(&self) -> &'static str {
        match self {
            FirstDay::Monday => "monday",
            FirstDay::Sunday => "sunday",
        }
    }
    pub const ALL: [FirstDay; 2] = [FirstDay::Monday, FirstDay::Sunday];
}
