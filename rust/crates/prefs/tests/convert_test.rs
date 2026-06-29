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
