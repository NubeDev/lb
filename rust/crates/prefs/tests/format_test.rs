//! Locale rendering + the format.quantity chart bridge (prefs scope). Number separators (`43,2` vs
//! `43.2`), date styles (eu/iso/usa), 12h/24h, and tz application over a stored UTC instant —
//! including a DST boundary (the hard part, delegated to `chrono-tz`).

use lb_prefs::{
    builtin, format_datetime, format_number, format_quantity, DateStyle, Dimension, NumberFormat,
    NumberOpts, TimeStyle, Unit,
};

#[test]
fn number_separators_per_format() {
    let n = 1234.5;
    assert_eq!(
        format_number(n, NumberFormat::DotComma, NumberOpts::default()),
        "1,234.5"
    );
    assert_eq!(
        format_number(n, NumberFormat::CommaDot, NumberOpts::default()),
        "1.234,5"
    );
    // 43,2 vs 43.2 — the scope's exact example.
    assert_eq!(
        format_number(43.2, NumberFormat::CommaDot, NumberOpts::default()),
        "43,2"
    );
    assert_eq!(
        format_number(43.2, NumberFormat::DotComma, NumberOpts::default()),
        "43.2"
    );
}

#[test]
fn number_max_frac_rounds() {
    let opts = NumberOpts { max_frac: Some(2) };
    assert_eq!(format_number(3.14159, NumberFormat::DotComma, opts), "3.14");
    // comma_dot: group sep '.', decimal sep ',', two fractional digits kept.
    assert_eq!(
        format_number(-1000.0, NumberFormat::CommaDot, opts),
        "-1.000,00"
    );
}

#[test]
fn date_styles() {
    // 2026-06-27 14:30 UTC.
    let ms = 1_782_570_600_000; // 2026-06-27T14:30:00Z
    let eu = format_datetime(ms, "UTC", DateStyle::Eu, TimeStyle::H24).unwrap();
    let iso = format_datetime(ms, "UTC", DateStyle::Iso, TimeStyle::H24).unwrap();
    let usa = format_datetime(ms, "UTC", DateStyle::Usa, TimeStyle::H12).unwrap();
    assert_eq!(eu, "27/06/2026 14:30");
    assert_eq!(iso, "2026-06-27 14:30");
    assert_eq!(usa, "06/27/2026 2:30 PM");
}

#[test]
fn timezone_applies_over_utc_instant() {
    let ms = 1_782_570_600_000; // 14:30Z
                                // Madrid is UTC+2 in summer (CEST) -> 16:30; New York is UTC-4 (EDT) -> 10:30.
    let madrid = format_datetime(ms, "Europe/Madrid", DateStyle::Eu, TimeStyle::H24).unwrap();
    let ny = format_datetime(ms, "America/New_York", DateStyle::Usa, TimeStyle::H12).unwrap();
    assert_eq!(madrid, "27/06/2026 16:30");
    assert_eq!(ny, "06/27/2026 10:30 AM");
}

#[test]
fn dst_boundary_is_handled() {
    // US "spring forward" 2026: 2026-03-08 07:00Z is 02:00 EST -> clocks jump to 03:00 EDT.
    // An instant just after the jump must render in EDT (UTC-4), not EST (UTC-5).
    let before = 1_772_953_200_000; // 2026-03-08T07:00:00Z  (== 02:00 EST, the instant of the jump)
    let after = before + 30 * 60 * 1000; // +30min -> 2026-03-08T07:30:00Z
    let ny_after =
        format_datetime(after, "America/New_York", DateStyle::Iso, TimeStyle::H24).unwrap();
    // 07:30Z in EDT (UTC-4) = 03:30.
    assert_eq!(ny_after, "2026-03-08 03:30");
    // An instant in January (EST, UTC-5) renders an hour further back for the same wall offset.
    let jan = 1_767_265_200_000; // 2026-01-01T11:00:00Z
    let ny_jan = format_datetime(jan, "America/New_York", DateStyle::Iso, TimeStyle::H24).unwrap();
    assert_eq!(ny_jan, "2026-01-01 06:00"); // 11:00Z - 5h = 06:00 EST
}

#[test]
fn unknown_timezone_errors() {
    assert!(format_datetime(0, "Mars/Olympus", DateStyle::Iso, TimeStyle::H24).is_err());
}

#[test]
fn format_quantity_es_metric_vs_en_knots() {
    // Example flow: 12 m/s wind. User A: es, metric -> "43,2 km/h". User B: en, imperial+knots
    // override -> "23.3 kn".
    let mut a = builtin();
    a.number_format = NumberFormat::CommaDot; // es decimal
    let qa = format_quantity(
        12.0,
        Unit::MeterPerSecond,
        Dimension::Speed,
        &a,
        NumberOpts::default(),
    )
    .unwrap();
    assert_eq!(qa.text, "43,2 km/h");

    let mut b = builtin();
    b.number_format = NumberFormat::DotComma;
    b.unit_system = lb_prefs::UnitSystem::Imperial;
    b.unit_overrides.insert(Dimension::Speed, Unit::Knot);
    let qb = format_quantity(
        12.0,
        Unit::MeterPerSecond,
        Dimension::Speed,
        &b,
        NumberOpts::default(),
    )
    .unwrap();
    assert_eq!(qb.text, "23.3 kn");
}

#[test]
fn format_quantity_rejects_mismatched_source_dimension() {
    let r = builtin();
    let err = format_quantity(
        20.0,
        Unit::Celsius,
        Dimension::Speed,
        &r,
        NumberOpts::default(),
    );
    assert!(err.is_err());
}
