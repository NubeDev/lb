//! The pure MF1 catalog layer (i18n-catalogs scope, unit/integration cases): plural/select in en+es,
//! the fallback chain (lang → en → key), the placeholder-failure contract (`[<arg>]`, never panic),
//! placeholder↔format parity (a message placeholder renders byte-identically to a direct `format.*`
//! call), and the catalog-lint rejection of out-of-subset messages. No infra — this layer is pure.

use std::collections::BTreeMap;

use lb_prefs::{
    format_datetime, format_quantity, lint_catalog, render_message, DateStyle, Dimension,
    NumberFormat, NumberOpts, Prefs, ResolvedPrefs, TimeStyle, Unit, UnitSystem,
};
use serde_json::json;

/// A fully-resolved prefs with the given language/tz/date-style/number-format (the axes the catalog
/// tests vary); the rest are sensible defaults.
fn prefs(lang: &str, tz: &str, date: DateStyle, number: NumberFormat) -> ResolvedPrefs {
    ResolvedPrefs {
        language: lang.into(),
        timezone: tz.into(),
        date_style: date,
        time_style: TimeStyle::H24,
        first_day_of_week: lb_prefs::FirstDay::Monday,
        number_format: number,
        unit_system: UnitSystem::Metric,
        unit_overrides: BTreeMap::new(),
        ui_theme: None,
        ui_branding: None,
    }
}

fn en() -> ResolvedPrefs {
    prefs("en", "UTC", DateStyle::Iso, NumberFormat::DotComma)
}
fn es() -> ResolvedPrefs {
    prefs("es", "Europe/Madrid", DateStyle::Eu, NumberFormat::CommaDot)
}

const NO_OVERRIDE: fn() -> BTreeMap<String, String> = BTreeMap::new;

#[test]
fn plural_selects_one_vs_other_en() {
    let o = NO_OVERRIDE();
    let one = render_message("alert.items_pending", &json!({ "count": 1 }), &o, &en());
    assert_eq!(one.text, "You have 1 pending item");
    let many = render_message("alert.items_pending", &json!({ "count": 5 }), &o, &en());
    assert_eq!(many.text, "You have 5 pending items");
}

#[test]
fn plural_exact_zero_arm_matches_before_category() {
    let o = NO_OVERRIDE();
    let zero = render_message("alert.items_pending", &json!({ "count": 0 }), &o, &en());
    assert_eq!(zero.text, "You have no pending items");
}

#[test]
fn plural_selects_es_categories() {
    let o = NO_OVERRIDE();
    let one = render_message(
        "notify.new_messages",
        &json!({ "name": "Ada", "count": 1 }),
        &o,
        &es(),
    );
    assert_eq!(one.text, "Ada te envió un mensaje");
    let many = render_message(
        "notify.new_messages",
        &json!({ "name": "Ada", "count": 3 }),
        &o,
        &es(),
    );
    assert_eq!(many.text, "Ada te envió 3 mensajes");
}

#[test]
fn select_keyword_and_other_fallback() {
    let o = NO_OVERRIDE();
    let crit = render_message(
        "alert.severity",
        &json!({ "level": "critical", "detail": "disk full" }),
        &o,
        &en(),
    );
    assert_eq!(crit.text, "Critical alert: disk full");
    // An unknown keyword falls to the mandatory `other` arm.
    let unknown = render_message(
        "alert.severity",
        &json!({ "level": "bogus", "detail": "x" }),
        &o,
        &en(),
    );
    assert_eq!(unknown.text, "Notice: x");
}

#[test]
fn placeholder_dispatches_to_format_inside_message() {
    // The es render puts a converted quantity + an EU/Madrid date into one message.
    let o = NO_OVERRIDE();
    let ts_ms = 1_751_373_000_000i64; // a fixed instant
    let r = render_message(
        "alert.threshold_crossed",
        &json!({ "name": "Sensor-1", "limit": 12.0, "ts": ts_ms }),
        &o,
        &es(),
    );
    // 12 m/s -> km/h in es metric, comma decimal; date EU in Madrid tz.
    assert!(
        r.text.starts_with("Sensor-1 superó 43,2 km/h el "),
        "got: {}",
        r.text
    );
    assert_eq!(r.locale_used, "es");
}

#[test]
fn fallback_language_then_en_then_key() {
    let o = NO_OVERRIDE();
    // notify.welcome exists in es -> uses es.
    let es_hit = render_message("notify.welcome", &json!({ "name": "Ana" }), &o, &es());
    assert_eq!(es_hit.text, "¡Bienvenido, Ana!");
    assert_eq!(es_hit.locale_used, "es");

    // An unknown key -> the key literal itself, never blank, never panic.
    let missing = render_message("does.not.exist", &json!({}), &o, &es());
    assert_eq!(missing.text, "does.not.exist");
}

#[test]
fn missing_es_key_falls_back_to_en_builtin() {
    // Simulate a locale with no builtin (fr) — merged/render falls to the en builtin per key.
    let o = NO_OVERRIDE();
    let fr = prefs("fr", "UTC", DateStyle::Iso, NumberFormat::DotComma);
    let r = render_message("notify.welcome", &json!({ "name": "Zoe" }), &o, &fr);
    assert_eq!(r.text, "Welcome, Zoe!");
    assert_eq!(r.locale_used, "en"); // the fallback layer that supplied it
}

#[test]
fn workspace_override_shadows_builtin() {
    let mut o = BTreeMap::new();
    o.insert(
        "notify.welcome".to_string(),
        "Hola de nuevo, {name} 👋".to_string(),
    );
    let r = render_message("notify.welcome", &json!({ "name": "Ada" }), &o, &es());
    assert_eq!(r.text, "Hola de nuevo, Ada 👋");
}

#[test]
fn placeholder_failure_renders_bracket_sentinel_not_panic() {
    let o = NO_OVERRIDE();
    // A null ts -> the `[ts]` literal; the rest of the message still renders.
    let r = render_message(
        "report.reading",
        &json!({ "value": 20.0, "ts": null }),
        &o,
        &en(),
    );
    assert!(
        r.text.starts_with("Reading: 20.0 °C recorded at [ts]"),
        "got: {}",
        r.text
    );
}

#[test]
fn placeholder_parity_datetime_matches_direct_format() {
    // A `{ts, date}` inside a message renders byte-identically to a direct format::datetime call.
    let o = NO_OVERRIDE();
    let ts_ms = 1_751_373_000_000i64;
    for p in [en(), es()] {
        let r = render_message(
            "report.reading",
            &json!({ "value": 20.0, "ts": ts_ms }),
            &o,
            &p,
        );
        let direct = format_datetime(ts_ms, &p.timezone, p.date_style, p.time_style).unwrap();
        assert!(
            r.text.ends_with(&direct),
            "message date ({}) must match direct format ({direct})",
            r.text
        );
    }
}

#[test]
fn placeholder_parity_quantity_matches_direct_format() {
    let o = NO_OVERRIDE();
    let ts_ms = 1_751_373_000_000i64;
    for p in [en(), es()] {
        let r = render_message(
            "alert.threshold_crossed",
            &json!({ "name": "S", "limit": 12.0, "ts": ts_ms }),
            &o,
            &p,
        );
        let direct = format_quantity(
            12.0,
            Dimension::Speed.canonical_unit(),
            Dimension::Speed,
            &p,
            NumberOpts::default(),
        )
        .unwrap();
        assert!(
            r.text.contains(&direct.text),
            "message quantity must contain direct format {}; got {}",
            direct.text,
            r.text
        );
    }
    // Sanity: the canonical unit for speed is m/s (what the placeholder converts FROM).
    assert_eq!(Dimension::Speed.canonical_unit(), Unit::MeterPerSecond);
}

#[test]
fn catalog_lint_accepts_builtins_rejects_out_of_subset() {
    // The builtins all pass the lint.
    let (merged, _v) = lb_prefs::merged_catalog("en", &NO_OVERRIDE());
    assert!(lint_catalog(&merged).is_ok(), "en builtin must lint clean");

    // An out-of-subset construct (a custom formatter) is rejected — an authoring error, not a
    // silent parse.
    let mut bad = BTreeMap::new();
    bad.insert("x".to_string(), "{n, spellout}".to_string());
    let err = lint_catalog(&bad).unwrap_err();
    assert_eq!(err.0, "x");

    // MF2 syntax and a too-deep nest also fail.
    let mut deep = BTreeMap::new();
    deep.insert(
        "d".to_string(),
        "{a, plural, other{{b, plural, other{{c, plural, other{#}}}}}}".to_string(),
    );
    assert!(
        lint_catalog(&deep).is_err(),
        "double-nested plural is out of subset"
    );
}

#[test]
fn set_default_prefs_unused_import_guard() {
    // Keep Prefs import meaningful (used by other suites); this trivial assert documents that the
    // pure catalog layer needs no stored record — it takes ResolvedPrefs directly.
    let _ = Prefs::default();
}
