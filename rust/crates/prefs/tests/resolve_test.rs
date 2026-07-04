//! Resolution chain + axis independence (prefs scope). request override > user > workspace default >
//! built-in, decided INDEPENDENTLY per axis; a base-locale seed fills only UNSET axes.

use std::collections::BTreeMap;

use lb_prefs::{resolve, DateStyle, Dimension, NumberFormat, Prefs, TimeStyle, Unit, UnitSystem};
use serde_json::json;

fn empty() -> Prefs {
    Prefs::default()
}

#[test]
fn builtin_when_chain_empty() {
    let r = resolve(&[]);
    assert_eq!(r.language, "en");
    assert_eq!(r.timezone, "UTC");
    assert_eq!(r.date_style, DateStyle::Iso);
    assert_eq!(r.unit_system, UnitSystem::Metric);
}

#[test]
fn each_level_of_the_chain_wins_in_order() {
    let ws_default = Prefs {
        timezone: Some("Europe/Madrid".into()),
        ..empty()
    };
    let user = Prefs {
        timezone: Some("America/New_York".into()),
        ..empty()
    };
    let override_ = Prefs {
        timezone: Some("Asia/Tokyo".into()),
        ..empty()
    };

    // workspace default only.
    assert_eq!(resolve(&[ws_default.clone()]).timezone, "Europe/Madrid");
    // user beats workspace default.
    assert_eq!(
        resolve(&[user.clone(), ws_default.clone()]).timezone,
        "America/New_York"
    );
    // request override beats user.
    assert_eq!(
        resolve(&[override_, user, ws_default]).timezone,
        "Asia/Tokyo"
    );
}

#[test]
fn axes_resolve_independently() {
    // language=es (user) + date_style=usa (override) + unit_system=metric (ws default) +
    // wind override=knots (user) — all coexist; none locks another.
    let mut user = Prefs {
        language: Some("es".into()),
        ..empty()
    };
    user.unit_overrides.insert(Dimension::Speed, Unit::Knot);
    let ws_default = Prefs {
        unit_system: Some(UnitSystem::Metric),
        number_format: Some(NumberFormat::DotComma),
        ..empty()
    };
    let override_ = Prefs {
        date_style: Some(DateStyle::Usa),
        ..empty()
    };

    let r = resolve(&[override_, user, ws_default]);
    assert_eq!(r.language, "es");
    assert_eq!(r.date_style, DateStyle::Usa);
    assert_eq!(r.unit_system, UnitSystem::Metric);
    assert_eq!(r.unit_overrides.get(&Dimension::Speed), Some(&Unit::Knot));
    // a non-overridden dimension still derives from the unit_system default.
    assert_eq!(r.display_unit(Dimension::Temperature), Unit::Celsius);
}

#[test]
fn base_locale_seed_fills_only_unset_axes() {
    // The user set ONLY language; a base-locale seed (es-ES region: comma decimals, EU dates, h24)
    // appended LAST fills the unset axes but never overrides the set one.
    let user = Prefs {
        date_style: Some(DateStyle::Usa),
        ..empty()
    };
    let seed = Prefs {
        language: Some("es".into()),
        date_style: Some(DateStyle::Eu),
        time_style: Some(TimeStyle::H24),
        number_format: Some(NumberFormat::CommaDot),
        ..empty()
    };
    let r = resolve(&[user, seed]);
    assert_eq!(r.date_style, DateStyle::Usa); // user's SET axis preserved
    assert_eq!(r.number_format, NumberFormat::CommaDot); // unset axis filled from seed
    assert_eq!(r.language, "es");
}

#[test]
fn unit_overrides_merge_per_dimension() {
    // user overrides speed; ws default overrides pressure — both survive (not all-or-nothing).
    let mut user = Prefs::default();
    user.unit_overrides.insert(Dimension::Speed, Unit::Knot);
    let mut ws_default = Prefs::default();
    ws_default
        .unit_overrides
        .insert(Dimension::Pressure, Unit::Psi);
    ws_default
        .unit_overrides
        .insert(Dimension::Speed, Unit::MilePerHour); // user shadows this

    let r = resolve(&[user, ws_default]);
    let mut want = BTreeMap::new();
    want.insert(Dimension::Speed, Unit::Knot);
    want.insert(Dimension::Pressure, Unit::Psi);
    assert_eq!(r.unit_overrides, want);
}

#[test]
fn disabled_language_falls_back_to_en() {
    // A language not compiled into this build resolves to the built-in fallback, never errors.
    let user = Prefs {
        language: Some("xx".into()),
        ..empty()
    };
    assert_eq!(resolve(&[user]).language, "en");
}

#[test]
fn ui_theme_folds_whole_first_link_wins() {
    // The theme blob is opaque and folds WHOLE — the highest-priority link that set it wins entirely,
    // it does not merge sub-fields. built-in is None.
    let member = Prefs {
        ui_theme: Some(json!({ "mode": "dark", "preset": "teal" })),
        ..empty()
    };
    let ws_default = Prefs {
        ui_theme: Some(json!({ "mode": "light", "preset": "amber" })),
        ..empty()
    };
    // [member, ws_default] — member wins whole.
    assert_eq!(
        resolve(&[member.clone(), ws_default.clone()]).ui_theme,
        Some(json!({ "mode": "dark", "preset": "teal" }))
    );
    // member unset → ws default fills in.
    assert_eq!(
        resolve(&[empty(), ws_default.clone()]).ui_theme,
        Some(json!({ "mode": "light", "preset": "amber" }))
    );
    // nothing set → None.
    assert_eq!(resolve(&[empty()]).ui_theme, None);
}
