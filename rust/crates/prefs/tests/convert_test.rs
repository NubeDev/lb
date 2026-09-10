//! Conversion correctness — the bug-prone part (prefs scope). °C↔°F is AFFINE (the classic trap);
//! speed/distance/mass round-trip; uom rejects a cross-dimension convert at the type level (here,
//! at our `Dimension` guard which mirrors uom's type-level guarantee — a temperature `Unit` and a
//! speed `Unit` can never reach the same uom quantity).

use lb_prefs::{convert, PrefsError, Unit};

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "expected {b}, got {a}");
}

#[test]
fn celsius_fahrenheit_is_affine() {
    // The two fixed points the scope names: 0°C = 32°F, 100°C = 212°F. A pure-scale (ratio) convert
    // would put 0°C at 0°F — the bug uom's affine ThermodynamicTemperature prevents.
    close(convert(0.0, Unit::Celsius, Unit::Fahrenheit).unwrap(), 32.0);
    close(
        convert(100.0, Unit::Celsius, Unit::Fahrenheit).unwrap(),
        212.0,
    );
    close(convert(32.0, Unit::Fahrenheit, Unit::Celsius).unwrap(), 0.0);
    close(
        convert(-40.0, Unit::Celsius, Unit::Fahrenheit).unwrap(),
        -40.0,
    ); // the crossover
    close(convert(0.0, Unit::Celsius, Unit::Kelvin).unwrap(), 273.15);
}

#[test]
fn celsius_round_trip_is_stable() {
    for c in [-40.0, 0.0, 21.5, 37.0, 100.0] {
        let f = convert(c, Unit::Celsius, Unit::Fahrenheit).unwrap();
        let back = convert(f, Unit::Fahrenheit, Unit::Celsius).unwrap();
        close(back, c);
    }
}

#[test]
fn speed_conversions() {
    // 12 m/s = 43.2 km/h = ~23.3289 knots (the scope's example-flow numbers).
    close(
        convert(12.0, Unit::MeterPerSecond, Unit::KilometerPerHour).unwrap(),
        43.2,
    );
    let kn = convert(12.0, Unit::MeterPerSecond, Unit::Knot).unwrap();
    assert!((kn - 23.33).abs() < 0.05, "knots ~23.3, got {kn}");
    // round-trip m/s -> km/h -> m/s
    let kmh = convert(12.0, Unit::MeterPerSecond, Unit::KilometerPerHour).unwrap();
    close(
        convert(kmh, Unit::KilometerPerHour, Unit::MeterPerSecond).unwrap(),
        12.0,
    );
}

#[test]
fn distance_conversions() {
    close(convert(1000.0, Unit::Meter, Unit::Kilometer).unwrap(), 1.0);
    let mi = convert(1609.344, Unit::Meter, Unit::Mile).unwrap();
    close(mi, 1.0);
    let ft = convert(1.0, Unit::Meter, Unit::Foot).unwrap();
    assert!((ft - 3.28084).abs() < 1e-4, "1 m ~ 3.28084 ft, got {ft}");
}

#[test]
fn percent_ratio() {
    close(convert(0.42, Unit::Ratio, Unit::Percent).unwrap(), 42.0);
    close(convert(42.0, Unit::Percent, Unit::Ratio).unwrap(), 0.42);
}

#[test]
fn cross_dimension_is_rejected() {
    // temperature -> speed is a hard error, never a silent number.
    let err = convert(20.0, Unit::Celsius, Unit::Knot).unwrap_err();
    assert!(
        matches!(err, PrefsError::CrossDimension { .. }),
        "got {err:?}"
    );
}

#[test]
fn every_unit_round_trips_within_its_dimension() {
    // For each unit, convert to its dimension's first sibling and back; magnitude is stable.
    for u in Unit::ALL {
        let sibling = Unit::ALL
            .iter()
            .copied()
            .find(|s| s.dimension() == u.dimension() && *s != u)
            .unwrap_or(u);
        let there = convert(7.5, u, sibling).unwrap();
        let back = convert(there, sibling, u).unwrap();
        close(back, 7.5);
    }
}

// ---------------------------------------------------------------------------
// The BAS vocabulary (energy/power/electrical/volume/flow/illuminance/
// concentration/frequency). These are the dimensions a building actually
// speaks; before this slice 59% of the unit-bearing panels on a real bench
// declared a unit lb could not convert at all.
// ---------------------------------------------------------------------------

#[test]
fn energy_kwh_is_the_meter_reading_unit() {
    // 1 kWh = 3.6 MJ — the identity every energy meter is calibrated against.
    close(
        convert(1.0, Unit::KilowattHour, Unit::Megajoule).unwrap(),
        3.6,
    );
    close(
        convert(1.0, Unit::KilowattHour, Unit::WattHour).unwrap(),
        1_000.0,
    );
    close(convert(1.0, Unit::WattHour, Unit::Joule).unwrap(), 3_600.0);
    close(
        convert(2.5, Unit::MegawattHour, Unit::KilowattHour).unwrap(),
        2_500.0,
    );
}

#[test]
fn power_and_electrical_scale() {
    close(convert(1.5, Unit::Kilowatt, Unit::Watt).unwrap(), 1_500.0);
    close(
        convert(240.0, Unit::Volt, Unit::Millivolt).unwrap(),
        240_000.0,
    );
    close(convert(11.0, Unit::Kilovolt, Unit::Volt).unwrap(), 11_000.0);
    // 4-20 mA is THE analogue instrumentation loop; it must land exactly.
    close(
        convert(20.0, Unit::Milliampere, Unit::Ampere).unwrap(),
        0.02,
    );
    close(
        convert(4.0, Unit::Milliampere, Unit::Ampere).unwrap(),
        0.004,
    );
}

#[test]
fn volume_and_flow_scale() {
    close(
        convert(1.0, Unit::CubicMeter, Unit::Liter).unwrap(),
        1_000.0,
    );
    close(
        convert(1.0, Unit::Kiloliter, Unit::CubicMeter).unwrap(),
        1.0,
    );
    close(
        convert(1.0, Unit::LiterPerSecond, Unit::LiterPerMinute).unwrap(),
        60.0,
    );
    close(
        convert(1.0, Unit::CubicMeterPerHour, Unit::LiterPerSecond).unwrap(),
        1_000.0 / 3_600.0,
    );
}

#[test]
fn concentration_and_frequency_scale() {
    close(
        convert(1.0, Unit::PartsPerMillion, Unit::PartsPerBillion).unwrap(),
        1_000.0,
    );
    // 400 ppm is outdoor-air CO2 — the baseline every IAQ panel is read against.
    close(
        convert(400_000.0, Unit::PartsPerBillion, Unit::PartsPerMillion).unwrap(),
        400.0,
    );
    close(convert(50.0, Unit::Hertz, Unit::Kilohertz).unwrap(), 0.05);
}

#[test]
fn kilopascal_joins_the_pressure_dimension() {
    // kPa was the one BAS pressure unit the original vocabulary missed.
    close(
        convert(1.0, Unit::Kilopascal, Unit::Pascal).unwrap(),
        1_000.0,
    );
    close(
        convert(101.325, Unit::Kilopascal, Unit::Hectopascal).unwrap(),
        1_013.25,
    );
}

/// **Apparent power is NOT power.** VA and W are dimensionally identical, so a naive uom wiring
/// would convert between them happily and return a number that is wrong by the power factor. They
/// are separate `Dimension`s precisely so this is a structural refusal, not a plausible answer.
#[test]
fn apparent_power_is_not_interconvertible_with_real_power() {
    let err = convert(100.0, Unit::VoltAmpere, Unit::Watt).unwrap_err();
    assert!(
        matches!(err, PrefsError::CrossDimension { .. }),
        "VA -> W must be refused, got {err:?}"
    );
    let err = convert(100.0, Unit::Kilowatt, Unit::KilovoltAmpere).unwrap_err();
    assert!(
        matches!(err, PrefsError::CrossDimension { .. }),
        "kW -> kVA must be refused, got {err:?}"
    );
    // Within apparent power it converts normally.
    close(
        convert(1.0, Unit::KilovoltAmpere, Unit::VoltAmpere).unwrap(),
        1_000.0,
    );
}

/// Energy and power are also distinct: kWh (an amount) and kW (a rate) are the single most
/// commonly conflated pair on an energy dashboard, and the review found panels declaring
/// `custom:kW/kWh` as one unit. The type system must refuse to bridge them.
#[test]
fn energy_is_not_power() {
    let err = convert(5.0, Unit::KilowattHour, Unit::Kilowatt).unwrap_err();
    assert!(
        matches!(err, PrefsError::CrossDimension { .. }),
        "kWh -> kW must be refused, got {err:?}"
    );
}

/// The units the review measured as unconvertible must now ALL parse and convert. This is the
/// test that fails if the vocabulary regresses.
#[test]
fn the_measured_bas_units_are_all_reachable() {
    for token in [
        "volt",
        "milliampere",
        "ampere",
        "watt",
        "kilowatt",
        "kilowatt_hour",
        "parts_per_million",
        "lux",
        "kiloliter",
        "volt_ampere",
        "hertz",
        "kilopascal",
    ] {
        let u = Unit::parse(token).unwrap_or_else(|| panic!("{token} must parse"));
        // Convertible to itself at minimum, and to a sibling if one exists.
        close(convert(1.0, u, u).unwrap(), 1.0);
    }
}
