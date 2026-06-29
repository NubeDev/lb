//! The **closed** physical-dimension enum — the named set a `unit_overrides` map may key on, and
//! the set `format.quantity`/`convert.unit` accept (prefs scope: "dimensions a named enum … never
//! open free text"). Adding a dimension is a deliberate change here, not an ad-hoc string.
//!
//! This enum is the single source of truth exported to the client as a generated constants module
//! (`bin/gen_ts`), so the prefs settings UI and the fieldConfig unit picker cannot disagree with
//! the server about the allowed set.

use serde::{Deserialize, Serialize};

/// A physical dimension a quantity can belong to. Closed: a value outside this set is rejected at
/// the type level (serde fails the deserialize), never silently passed through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dimension {
    Temperature,
    Speed,
    Distance,
    Mass,
    Pressure,
    Data,
    Percent,
    Time,
}

impl Dimension {
    /// The wire/serde token (`snake_case`) — the same string the generated TS constants carry.
    pub fn as_str(&self) -> &'static str {
        match self {
            Dimension::Temperature => "temperature",
            Dimension::Speed => "speed",
            Dimension::Distance => "distance",
            Dimension::Mass => "mass",
            Dimension::Pressure => "pressure",
            Dimension::Data => "data",
            Dimension::Percent => "percent",
            Dimension::Time => "time",
        }
    }

    /// Every dimension, in declaration order — drives the generated client constants and exhaustive
    /// tests. Keep in sync with the enum (a test asserts the count).
    pub const ALL: [Dimension; 8] = [
        Dimension::Temperature,
        Dimension::Speed,
        Dimension::Distance,
        Dimension::Mass,
        Dimension::Pressure,
        Dimension::Data,
        Dimension::Percent,
        Dimension::Time,
    ];
}
