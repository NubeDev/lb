//! The **closed** unit enum — every unit the platform can convert to/from, each tied to exactly one
//! [`Dimension`]. The wire token (`as_str`) is the canonical unit name used by the `unit:` tag
//! (tags scope) and by `format.quantity(value, from_unit, dimension)`. An unknown unit is a hard
//! error, never a passthrough (prefs scope risk: "source-unit provenance").
//!
//! Conversion math lives in `convert/` (uom-backed); this file is only the *vocabulary* + which
//! dimension each unit belongs to, so a cross-dimension convert can be rejected structurally.

use serde::{Deserialize, Serialize};

use super::dimension::Dimension;

/// A unit of measure. Closed and named; the token is locale-neutral and stable (it is what a
/// `unit:` tag stores). Grouped by dimension in declaration order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    // temperature (affine!)
    Celsius,
    Fahrenheit,
    Kelvin,
    // speed
    MeterPerSecond,
    KilometerPerHour,
    MilePerHour,
    Knot,
    // distance
    Meter,
    Kilometer,
    Foot,
    Mile,
    // mass
    Kilogram,
    Gram,
    Pound,
    Ounce,
    // pressure
    Pascal,
    Hectopascal,
    Bar,
    Psi,
    // data
    Byte,
    Kilobyte,
    Megabyte,
    Gigabyte,
    // percent (dimensionless ratio rendered as %)
    Ratio,
    Percent,
    // time (duration)
    Second,
    Minute,
    Hour,
    Day,
}

impl Unit {
    /// The dimension this unit measures — a `convert`/`format.quantity` across dimensions is
    /// rejected because the two `Unit`s report different `Dimension`s.
    pub fn dimension(&self) -> Dimension {
        use Unit::*;
        match self {
            Celsius | Fahrenheit | Kelvin => Dimension::Temperature,
            MeterPerSecond | KilometerPerHour | MilePerHour | Knot => Dimension::Speed,
            Meter | Kilometer | Foot | Mile => Dimension::Distance,
            Kilogram | Gram | Pound | Ounce => Dimension::Mass,
            Pascal | Hectopascal | Bar | Psi => Dimension::Pressure,
            Byte | Kilobyte | Megabyte | Gigabyte => Dimension::Data,
            Ratio | Percent => Dimension::Percent,
            Second | Minute | Hour | Day => Dimension::Time,
        }
    }

    /// The wire/serde token (`snake_case`).
    pub fn as_str(&self) -> &'static str {
        use Unit::*;
        match self {
            Celsius => "celsius",
            Fahrenheit => "fahrenheit",
            Kelvin => "kelvin",
            MeterPerSecond => "meter_per_second",
            KilometerPerHour => "kilometer_per_hour",
            MilePerHour => "mile_per_hour",
            Knot => "knot",
            Meter => "meter",
            Kilometer => "kilometer",
            Foot => "foot",
            Mile => "mile",
            Kilogram => "kilogram",
            Gram => "gram",
            Pound => "pound",
            Ounce => "ounce",
            Pascal => "pascal",
            Hectopascal => "hectopascal",
            Bar => "bar",
            Psi => "psi",
            Byte => "byte",
            Kilobyte => "kilobyte",
            Megabyte => "megabyte",
            Gigabyte => "gigabyte",
            Ratio => "ratio",
            Percent => "percent",
            Second => "second",
            Minute => "minute",
            Hour => "hour",
            Day => "day",
        }
    }

    /// A short localized-display abbreviation used when rendering (`43.2 km/h`). en/es share these
    /// SI/common abbreviations; a future per-locale unit-name table is a follow-up (the scope's
    /// "localized unit display name via icu4x" — abbreviations are locale-stable enough for v1).
    pub fn abbrev(&self) -> &'static str {
        use Unit::*;
        match self {
            Celsius => "°C",
            Fahrenheit => "°F",
            Kelvin => "K",
            MeterPerSecond => "m/s",
            KilometerPerHour => "km/h",
            MilePerHour => "mph",
            Knot => "kn",
            Meter => "m",
            Kilometer => "km",
            Foot => "ft",
            Mile => "mi",
            Kilogram => "kg",
            Gram => "g",
            Pound => "lb",
            Ounce => "oz",
            Pascal => "Pa",
            Hectopascal => "hPa",
            Bar => "bar",
            Psi => "psi",
            Byte => "B",
            Kilobyte => "kB",
            Megabyte => "MB",
            Gigabyte => "GB",
            Ratio => "",
            Percent => "%",
            Second => "s",
            Minute => "min",
            Hour => "h",
            Day => "d",
        }
    }

    /// Parse a wire token into a `Unit`; `None` for anything outside the closed set (the caller
    /// raises [`crate::PrefsError::UnknownUnit`] — never a guess).
    pub fn parse(token: &str) -> Option<Unit> {
        Unit::ALL.iter().copied().find(|u| u.as_str() == token)
    }

    /// Every unit, in declaration order — drives the generated client constants and exhaustive
    /// round-trip tests.
    pub const ALL: [Unit; 29] = [
        Unit::Celsius,
        Unit::Fahrenheit,
        Unit::Kelvin,
        Unit::MeterPerSecond,
        Unit::KilometerPerHour,
        Unit::MilePerHour,
        Unit::Knot,
        Unit::Meter,
        Unit::Kilometer,
        Unit::Foot,
        Unit::Mile,
        Unit::Kilogram,
        Unit::Gram,
        Unit::Pound,
        Unit::Ounce,
        Unit::Pascal,
        Unit::Hectopascal,
        Unit::Bar,
        Unit::Psi,
        Unit::Byte,
        Unit::Kilobyte,
        Unit::Megabyte,
        Unit::Gigabyte,
        Unit::Ratio,
        Unit::Percent,
        Unit::Second,
        Unit::Minute,
        Unit::Hour,
        Unit::Day,
    ];
}
