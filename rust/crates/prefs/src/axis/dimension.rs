//! The **closed** physical-dimension enum — the named set a `unit_overrides` map may key on, and
//! the set `format.quantity`/`convert.unit` accept (prefs scope: "dimensions a named enum … never
//! open free text"). Adding a dimension is a deliberate change here, not an ad-hoc string.
//!
//! This enum is the single source of truth exported to the client as a generated constants module
//! (`bin/gen_ts`), so the prefs settings UI and the fieldConfig unit picker cannot disagree with
//! the server about the allowed set.

use serde::{Deserialize, Serialize};

use super::unit::Unit;

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

    /// The **canonical (base) unit** a stored value of this dimension is expressed in — the SI/base
    /// the platform stores canonically (prefs scope: "UTC instants, SI/base units"). A `{v, quantity,
    /// <dim>}` catalog placeholder carries the *canonical* value, so this is the `from_unit` the
    /// renderer converts from before applying the recipient's display unit. (Kept beside the closed
    /// enum so the two never drift.)
    pub fn canonical_unit(&self) -> Unit {
        match self {
            Dimension::Temperature => Unit::Celsius,
            Dimension::Speed => Unit::MeterPerSecond,
            Dimension::Distance => Unit::Meter,
            Dimension::Mass => Unit::Kilogram,
            Dimension::Pressure => Unit::Pascal,
            Dimension::Data => Unit::Byte,
            Dimension::Percent => Unit::Ratio,
            Dimension::Time => Unit::Second,
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
