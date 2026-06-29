//! `convert(value, from, to)` — dimensionally-sound unit conversion, **uom-backed** so the
//! correctness (the bug-prone part) is the type system's job, not a hand-rolled factor table
//! (prefs scope risk: "°C↔°F is affine; a factor map gets temperature wrong").
//!
//! A cross-dimension convert (temperature → speed) is rejected up front because the two `Unit`s
//! report different `Dimension`s — the structural guarantee the scope's type-level-rejection test
//! asserts. Within a dimension we route to the matching uom quantity, set the value in the `from`
//! unit, and read it back in the `to` unit; uom applies the conversion (including the affine
//! offset+scale for thermodynamic temperature) internally.

use uom::si::f64::{Information, Length, Mass, Pressure, ThermodynamicTemperature, Time, Velocity};
use uom::si::information::{byte, gigabyte, kilobyte, megabyte};
use uom::si::length::{foot, kilometer, meter, mile};
use uom::si::mass::{gram, kilogram, ounce, pound};
use uom::si::pressure::{bar, hectopascal, pascal, psi};
use uom::si::thermodynamic_temperature::{degree_celsius, degree_fahrenheit, kelvin};
use uom::si::time::{day, hour, minute, second};
use uom::si::velocity::{kilometer_per_hour, knot, meter_per_second, mile_per_hour};

use crate::axis::{Dimension, Unit};
use crate::error::PrefsError;

/// Convert `value` expressed in `from` into `to`. Both must share a dimension; otherwise
/// [`PrefsError::CrossDimension`]. Percent is a pure ratio (`ratio` 0..1 ↔ `percent` 0..100).
pub fn convert(value: f64, from: Unit, to: Unit) -> Result<f64, PrefsError> {
    if from.dimension() != to.dimension() {
        return Err(PrefsError::CrossDimension {
            from: from.as_str(),
            from_dim: from.dimension().as_str(),
            to: to.as_str(),
            to_dim: to.dimension().as_str(),
        });
    }
    Ok(match from.dimension() {
        Dimension::Temperature => temperature(value, from, to),
        Dimension::Speed => speed(value, from, to),
        Dimension::Distance => distance(value, from, to),
        Dimension::Mass => mass(value, from, to),
        Dimension::Pressure => pressure(value, from, to),
        Dimension::Data => data(value, from, to),
        Dimension::Time => time(value, from, to),
        Dimension::Percent => percent(value, from, to),
    })
}

fn temperature(v: f64, from: Unit, to: Unit) -> f64 {
    let q = match from {
        Unit::Celsius => ThermodynamicTemperature::new::<degree_celsius>(v),
        Unit::Fahrenheit => ThermodynamicTemperature::new::<degree_fahrenheit>(v),
        Unit::Kelvin => ThermodynamicTemperature::new::<kelvin>(v),
        _ => unreachable!("temperature dimension guarantees a temperature unit"),
    };
    match to {
        Unit::Celsius => q.get::<degree_celsius>(),
        Unit::Fahrenheit => q.get::<degree_fahrenheit>(),
        Unit::Kelvin => q.get::<kelvin>(),
        _ => unreachable!(),
    }
}

fn speed(v: f64, from: Unit, to: Unit) -> f64 {
    let q = match from {
        Unit::MeterPerSecond => Velocity::new::<meter_per_second>(v),
        Unit::KilometerPerHour => Velocity::new::<kilometer_per_hour>(v),
        Unit::MilePerHour => Velocity::new::<mile_per_hour>(v),
        Unit::Knot => Velocity::new::<knot>(v),
        _ => unreachable!(),
    };
    match to {
        Unit::MeterPerSecond => q.get::<meter_per_second>(),
        Unit::KilometerPerHour => q.get::<kilometer_per_hour>(),
        Unit::MilePerHour => q.get::<mile_per_hour>(),
        Unit::Knot => q.get::<knot>(),
        _ => unreachable!(),
    }
}

fn distance(v: f64, from: Unit, to: Unit) -> f64 {
    let q = match from {
        Unit::Meter => Length::new::<meter>(v),
        Unit::Kilometer => Length::new::<kilometer>(v),
        Unit::Foot => Length::new::<foot>(v),
        Unit::Mile => Length::new::<mile>(v),
        _ => unreachable!(),
    };
    match to {
        Unit::Meter => q.get::<meter>(),
        Unit::Kilometer => q.get::<kilometer>(),
        Unit::Foot => q.get::<foot>(),
        Unit::Mile => q.get::<mile>(),
        _ => unreachable!(),
    }
}

fn mass(v: f64, from: Unit, to: Unit) -> f64 {
    let q = match from {
        Unit::Kilogram => Mass::new::<kilogram>(v),
        Unit::Gram => Mass::new::<gram>(v),
        Unit::Pound => Mass::new::<pound>(v),
        Unit::Ounce => Mass::new::<ounce>(v),
        _ => unreachable!(),
    };
    match to {
        Unit::Kilogram => q.get::<kilogram>(),
        Unit::Gram => q.get::<gram>(),
        Unit::Pound => q.get::<pound>(),
        Unit::Ounce => q.get::<ounce>(),
        _ => unreachable!(),
    }
}

fn pressure(v: f64, from: Unit, to: Unit) -> f64 {
    let q = match from {
        Unit::Pascal => Pressure::new::<pascal>(v),
        Unit::Hectopascal => Pressure::new::<hectopascal>(v),
        Unit::Bar => Pressure::new::<bar>(v),
        Unit::Psi => Pressure::new::<psi>(v),
        _ => unreachable!(),
    };
    match to {
        Unit::Pascal => q.get::<pascal>(),
        Unit::Hectopascal => q.get::<hectopascal>(),
        Unit::Bar => q.get::<bar>(),
        Unit::Psi => q.get::<psi>(),
        _ => unreachable!(),
    }
}

fn data(v: f64, from: Unit, to: Unit) -> f64 {
    let q = match from {
        Unit::Byte => Information::new::<byte>(v),
        Unit::Kilobyte => Information::new::<kilobyte>(v),
        Unit::Megabyte => Information::new::<megabyte>(v),
        Unit::Gigabyte => Information::new::<gigabyte>(v),
        _ => unreachable!(),
    };
    match to {
        Unit::Byte => q.get::<byte>(),
        Unit::Kilobyte => q.get::<kilobyte>(),
        Unit::Megabyte => q.get::<megabyte>(),
        Unit::Gigabyte => q.get::<gigabyte>(),
        _ => unreachable!(),
    }
}

fn time(v: f64, from: Unit, to: Unit) -> f64 {
    let q = match from {
        Unit::Second => Time::new::<second>(v),
        Unit::Minute => Time::new::<minute>(v),
        Unit::Hour => Time::new::<hour>(v),
        Unit::Day => Time::new::<day>(v),
        _ => unreachable!(),
    };
    match to {
        Unit::Second => q.get::<second>(),
        Unit::Minute => q.get::<minute>(),
        Unit::Hour => q.get::<hour>(),
        Unit::Day => q.get::<day>(),
        _ => unreachable!(),
    }
}

/// Percent is a pure dimensionless ratio — uom's `Ratio` would also serve, but the scale is trivial
/// and explicit here: `ratio` is 0..1, `percent` is 0..100.
fn percent(v: f64, from: Unit, to: Unit) -> f64 {
    let as_ratio = match from {
        Unit::Ratio => v,
        Unit::Percent => v / 100.0,
        _ => unreachable!(),
    };
    match to {
        Unit::Ratio => as_ratio,
        Unit::Percent => as_ratio * 100.0,
        _ => unreachable!(),
    }
}
