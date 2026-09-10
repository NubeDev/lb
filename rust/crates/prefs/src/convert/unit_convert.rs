//! `convert(value, from, to)` — dimensionally-sound unit conversion, **uom-backed** so the
//! correctness (the bug-prone part) is the type system's job, not a hand-rolled factor table
//! (prefs scope risk: "°C↔°F is affine; a factor map gets temperature wrong").
//!
//! A cross-dimension convert (temperature → speed) is rejected up front because the two `Unit`s
//! report different `Dimension`s — the structural guarantee the scope's type-level-rejection test
//! asserts. Within a dimension we route to the matching uom quantity, set the value in the `from`
//! unit, and read it back in the `to` unit; uom applies the conversion (including the affine
//! offset+scale for thermodynamic temperature) internally.

use uom::si::electric_current::{ampere, milliampere};
use uom::si::electric_potential::{kilovolt, millivolt, volt};
use uom::si::energy::{joule, kilojoule, kilowatt_hour, megajoule, megawatt_hour, watt_hour};
use uom::si::f64::{
    ElectricCurrent, ElectricPotential, Energy, Frequency, Illuminance, Information, Length, Mass,
    Power, Pressure, ThermodynamicTemperature, Time, Velocity, Volume, VolumeRate,
};
use uom::si::frequency::{hertz, kilohertz};
use uom::si::illuminance::lux;
use uom::si::information::{byte, gigabyte, kilobyte, megabyte};
use uom::si::length::{foot, kilometer, meter, mile};
use uom::si::mass::{gram, kilogram, ounce, pound};
use uom::si::power::{kilowatt, megawatt, watt};
use uom::si::pressure::{bar, hectopascal, kilopascal, pascal, psi};
use uom::si::thermodynamic_temperature::{degree_celsius, degree_fahrenheit, kelvin};
use uom::si::time::{day, hour, minute, second};
use uom::si::velocity::{kilometer_per_hour, knot, meter_per_second, mile_per_hour};
use uom::si::volume::{cubic_meter, kiloliter, liter};
use uom::si::volume_rate::{
    cubic_meter_per_hour, cubic_meter_per_second, liter_per_minute, liter_per_second,
};

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
        Dimension::Energy => energy(value, from, to),
        Dimension::Power => power(value, from, to),
        Dimension::ElectricPotential => electric_potential(value, from, to),
        Dimension::ElectricCurrent => electric_current(value, from, to),
        Dimension::ApparentPower => apparent_power(value, from, to),
        Dimension::Volume => volume(value, from, to),
        Dimension::VolumeFlow => volume_flow(value, from, to),
        Dimension::Illuminance => illuminance(value, from, to),
        Dimension::Concentration => concentration(value, from, to),
        Dimension::Frequency => frequency(value, from, to),
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
        Unit::Kilopascal => Pressure::new::<kilopascal>(v),
        Unit::Bar => Pressure::new::<bar>(v),
        Unit::Psi => Pressure::new::<psi>(v),
        _ => unreachable!(),
    };
    match to {
        Unit::Pascal => q.get::<pascal>(),
        Unit::Hectopascal => q.get::<hectopascal>(),
        Unit::Kilopascal => q.get::<kilopascal>(),
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

fn energy(v: f64, from: Unit, to: Unit) -> f64 {
    let q = match from {
        Unit::Joule => Energy::new::<joule>(v),
        Unit::Kilojoule => Energy::new::<kilojoule>(v),
        Unit::Megajoule => Energy::new::<megajoule>(v),
        Unit::WattHour => Energy::new::<watt_hour>(v),
        Unit::KilowattHour => Energy::new::<kilowatt_hour>(v),
        Unit::MegawattHour => Energy::new::<megawatt_hour>(v),
        _ => unreachable!(),
    };
    match to {
        Unit::Joule => q.get::<joule>(),
        Unit::Kilojoule => q.get::<kilojoule>(),
        Unit::Megajoule => q.get::<megajoule>(),
        Unit::WattHour => q.get::<watt_hour>(),
        Unit::KilowattHour => q.get::<kilowatt_hour>(),
        Unit::MegawattHour => q.get::<megawatt_hour>(),
        _ => unreachable!(),
    }
}

fn power(v: f64, from: Unit, to: Unit) -> f64 {
    let q = match from {
        Unit::Watt => Power::new::<watt>(v),
        Unit::Kilowatt => Power::new::<kilowatt>(v),
        Unit::Megawatt => Power::new::<megawatt>(v),
        _ => unreachable!(),
    };
    match to {
        Unit::Watt => q.get::<watt>(),
        Unit::Kilowatt => q.get::<kilowatt>(),
        Unit::Megawatt => q.get::<megawatt>(),
        _ => unreachable!(),
    }
}

fn electric_potential(v: f64, from: Unit, to: Unit) -> f64 {
    let q = match from {
        Unit::Volt => ElectricPotential::new::<volt>(v),
        Unit::Millivolt => ElectricPotential::new::<millivolt>(v),
        Unit::Kilovolt => ElectricPotential::new::<kilovolt>(v),
        _ => unreachable!(),
    };
    match to {
        Unit::Volt => q.get::<volt>(),
        Unit::Millivolt => q.get::<millivolt>(),
        Unit::Kilovolt => q.get::<kilovolt>(),
        _ => unreachable!(),
    }
}

fn electric_current(v: f64, from: Unit, to: Unit) -> f64 {
    let q = match from {
        Unit::Ampere => ElectricCurrent::new::<ampere>(v),
        Unit::Milliampere => ElectricCurrent::new::<milliampere>(v),
        _ => unreachable!(),
    };
    match to {
        Unit::Ampere => q.get::<ampere>(),
        Unit::Milliampere => q.get::<milliampere>(),
        _ => unreachable!(),
    }
}

/// Apparent power (VA) is its OWN dimension on purpose. It is dimensionally identical to real power
/// (W) — uom cannot tell them apart, and would happily convert VA to W — but the two are related by
/// a power factor the platform does not know. Keeping them separate makes `VA → W` a structural
/// [`PrefsError::CrossDimension`] rather than a plausible wrong number. So the scale is explicit
/// here (kVA is 1000 VA) rather than routed through uom.
fn apparent_power(v: f64, from: Unit, to: Unit) -> f64 {
    let as_va = match from {
        Unit::VoltAmpere => v,
        Unit::KilovoltAmpere => v * 1_000.0,
        _ => unreachable!(),
    };
    match to {
        Unit::VoltAmpere => as_va,
        Unit::KilovoltAmpere => as_va / 1_000.0,
        _ => unreachable!(),
    }
}

fn volume(v: f64, from: Unit, to: Unit) -> f64 {
    let q = match from {
        Unit::CubicMeter => Volume::new::<cubic_meter>(v),
        Unit::Liter => Volume::new::<liter>(v),
        Unit::Kiloliter => Volume::new::<kiloliter>(v),
        _ => unreachable!(),
    };
    match to {
        Unit::CubicMeter => q.get::<cubic_meter>(),
        Unit::Liter => q.get::<liter>(),
        Unit::Kiloliter => q.get::<kiloliter>(),
        _ => unreachable!(),
    }
}

fn volume_flow(v: f64, from: Unit, to: Unit) -> f64 {
    let q = match from {
        Unit::CubicMeterPerSecond => VolumeRate::new::<cubic_meter_per_second>(v),
        Unit::CubicMeterPerHour => VolumeRate::new::<cubic_meter_per_hour>(v),
        Unit::LiterPerSecond => VolumeRate::new::<liter_per_second>(v),
        Unit::LiterPerMinute => VolumeRate::new::<liter_per_minute>(v),
        _ => unreachable!(),
    };
    match to {
        Unit::CubicMeterPerSecond => q.get::<cubic_meter_per_second>(),
        Unit::CubicMeterPerHour => q.get::<cubic_meter_per_hour>(),
        Unit::LiterPerSecond => q.get::<liter_per_second>(),
        Unit::LiterPerMinute => q.get::<liter_per_minute>(),
        _ => unreachable!(),
    }
}

fn illuminance(v: f64, from: Unit, to: Unit) -> f64 {
    let q = match from {
        Unit::Lux => Illuminance::new::<lux>(v),
        _ => unreachable!(),
    };
    match to {
        Unit::Lux => q.get::<lux>(),
        _ => unreachable!(),
    }
}

/// Concentration as a pure ratio scale (ppm/ppb), like [`percent`]. uom's `Ratio` would also serve;
/// the scale is trivial and explicit is clearer.
fn concentration(v: f64, from: Unit, to: Unit) -> f64 {
    let as_ppm = match from {
        Unit::PartsPerMillion => v,
        Unit::PartsPerBillion => v / 1_000.0,
        _ => unreachable!(),
    };
    match to {
        Unit::PartsPerMillion => as_ppm,
        Unit::PartsPerBillion => as_ppm * 1_000.0,
        _ => unreachable!(),
    }
}

fn frequency(v: f64, from: Unit, to: Unit) -> f64 {
    let q = match from {
        Unit::Hertz => Frequency::new::<hertz>(v),
        Unit::Kilohertz => Frequency::new::<kilohertz>(v),
        _ => unreachable!(),
    };
    match to {
        Unit::Hertz => q.get::<hertz>(),
        Unit::Kilohertz => q.get::<kilohertz>(),
        _ => unreachable!(),
    }
}
