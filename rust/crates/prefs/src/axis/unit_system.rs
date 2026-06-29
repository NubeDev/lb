//! `unit_system` axis — metric vs imperial (prefs scope, closed set). It supplies the *default*
//! display unit per dimension; a `unit_overrides` entry shadows it for one dimension (e.g. metric
//! everywhere but wind in knots).

use serde::{Deserialize, Serialize};

use super::dimension::Dimension;
use super::unit::Unit;

/// The base measurement system. `metric` (SI-leaning) or `imperial` (US customary).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnitSystem {
    Metric,
    Imperial,
}

impl UnitSystem {
    pub fn as_str(&self) -> &'static str {
        match self {
            UnitSystem::Metric => "metric",
            UnitSystem::Imperial => "imperial",
        }
    }
    pub const ALL: [UnitSystem; 2] = [UnitSystem::Metric, UnitSystem::Imperial];

    /// The display unit this system uses for `dimension`, absent an override. This is the table a
    /// `unit_overrides.<dimension>` entry shadows. Temperature/speed/distance/mass differ by system;
    /// dimensionless and storage dimensions (percent/data/time) are system-invariant.
    pub fn default_unit(&self, dimension: Dimension) -> Unit {
        match (self, dimension) {
            (_, Dimension::Percent) => Unit::Percent,
            (_, Dimension::Time) => Unit::Second,
            (_, Dimension::Data) => Unit::Megabyte,

            (UnitSystem::Metric, Dimension::Temperature) => Unit::Celsius,
            (UnitSystem::Imperial, Dimension::Temperature) => Unit::Fahrenheit,

            (UnitSystem::Metric, Dimension::Speed) => Unit::KilometerPerHour,
            (UnitSystem::Imperial, Dimension::Speed) => Unit::MilePerHour,

            (UnitSystem::Metric, Dimension::Distance) => Unit::Kilometer,
            (UnitSystem::Imperial, Dimension::Distance) => Unit::Mile,

            (UnitSystem::Metric, Dimension::Mass) => Unit::Kilogram,
            (UnitSystem::Imperial, Dimension::Mass) => Unit::Pound,

            (UnitSystem::Metric, Dimension::Pressure) => Unit::Hectopascal,
            (UnitSystem::Imperial, Dimension::Pressure) => Unit::Psi,
        }
    }
}
