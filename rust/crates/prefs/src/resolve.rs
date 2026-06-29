//! `resolve` — the pure fold over the preference chain (prefs scope, the core function):
//!
//! ```text
//! request override  →  user pref  →  workspace default  →  built-in fallback (en, UTC, iso, h24, metric)
//! ```
//!
//! **Each axis is resolved INDEPENDENTLY** (the decouple-the-axes insight): `language=es` from the
//! user, `date_style=usa` from a request override, and `unit_system=metric` from the workspace
//! default all coexist. A base-locale "seed" is just another (lowest-priority, pre-fallback) `Prefs`
//! the caller can splice in to fill *only the unset* axes — it never overrides a set one, because a
//! set axis is taken from a higher link first.
//!
//! `unit_overrides` folds per-dimension, not all-or-nothing: an override map merges so the user can
//! set `wind_speed=knots` while the workspace default set `pressure=psi` and both survive.

use std::collections::BTreeMap;

use crate::axis::language;
use crate::axis::{DateStyle, NumberFormat, TimeStyle, UnitSystem};
use crate::prefs::{Prefs, ResolvedPrefs};

/// The built-in fallback — the last link, always fully populated (prefs scope: `en, UTC, iso,
/// metric`; 24h and Monday are the metric-region seeds).
pub fn builtin() -> ResolvedPrefs {
    ResolvedPrefs {
        language: language::FALLBACK.to_string(),
        timezone: "UTC".to_string(),
        date_style: DateStyle::Iso,
        time_style: TimeStyle::H24,
        first_day_of_week: crate::axis::FirstDay::Monday,
        number_format: NumberFormat::DotComma,
        unit_system: UnitSystem::Metric,
        unit_overrides: BTreeMap::new(),
    }
}

/// Fold the chain into a fully-decided [`ResolvedPrefs`]. `links` is highest-priority first
/// (e.g. `[request_override, user, workspace_default]` or with a base-locale seed appended last);
/// the built-in fallback is implicit and lowest. A `None` axis in a link defers to the next link.
///
/// Pure: no I/O, no clock, no store. The host loads the records and calls this; a `format.*` verb
/// with a request override prepends a one-axis `Prefs` and re-folds.
pub fn resolve(links: &[Prefs]) -> ResolvedPrefs {
    let base = builtin();

    // Each axis: first link that has it Some, else the built-in. Independent per axis.
    let language = first(links, |p| p.language.clone())
        .filter(|l| language::is_enabled(l))
        .unwrap_or(base.language);
    let timezone = first(links, |p| p.timezone.clone()).unwrap_or(base.timezone);
    let date_style = first(links, |p| p.date_style).unwrap_or(base.date_style);
    let time_style = first(links, |p| p.time_style).unwrap_or(base.time_style);
    let first_day_of_week = first(links, |p| p.first_day_of_week).unwrap_or(base.first_day_of_week);
    let number_format = first(links, |p| p.number_format).unwrap_or(base.number_format);
    let unit_system = first(links, |p| p.unit_system).unwrap_or(base.unit_system);

    // unit_overrides merges per-dimension, highest-priority link wins each key. Iterate
    // lowest→highest so a higher link overwrites; start empty (built-in has none).
    let mut unit_overrides = BTreeMap::new();
    for link in links.iter().rev() {
        for (dim, unit) in &link.unit_overrides {
            unit_overrides.insert(*dim, *unit);
        }
    }

    ResolvedPrefs {
        language,
        timezone,
        date_style,
        time_style,
        first_day_of_week,
        number_format,
        unit_system,
        unit_overrides,
    }
}

/// The first link (highest priority first) for which `pick` returns `Some`.
fn first<T>(links: &[Prefs], pick: impl Fn(&Prefs) -> Option<T>) -> Option<T> {
    links.iter().find_map(pick)
}
