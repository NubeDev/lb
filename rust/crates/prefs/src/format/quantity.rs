//! `format.quantity(value, from_unit, dimension, opts?)` — the chart-formatting bridge (prefs scope;
//! the fieldConfig consumer). It composes the two halves: **uom** converts the canonical value to
//! the user's display unit, **then** the number formatter renders the magnitude with the locale's
//! separators and the unit's abbreviation is appended (`12.0 m/s` → `43,2 km/h` for an es user,
//! `23.3 kn` for an en user with a wind→knots override).

use crate::axis::{Dimension, Unit};
use crate::convert::to_display;
use crate::error::PrefsError;
use crate::prefs::ResolvedPrefs;

use super::number::{format_number, NumberOpts};

/// The fully-rendered quantity string + the structured parts (so a caller that wants the number and
/// unit separately — e.g. a chart axis — does not re-parse the string).
pub struct FormattedQuantity {
    pub text: String,
    pub value: f64,
    pub unit: Unit,
}

/// Convert `value` (in `from_unit`, a `dimension` quantity) to `resolved`'s display unit and render
/// it. `max_frac` defaults to 1 decimal (the common chart case) when `opts.max_frac` is `None`.
pub fn format_quantity(
    value: f64,
    from_unit: Unit,
    dimension: Dimension,
    resolved: &ResolvedPrefs,
    opts: NumberOpts,
) -> Result<FormattedQuantity, PrefsError> {
    let display = to_display(value, from_unit, dimension, resolved)?;
    let opts = NumberOpts {
        max_frac: opts.max_frac.or(Some(1)),
    };
    let number = format_number(display.value, resolved.number_format, opts);
    let abbrev = display.unit.abbrev();
    let text = if abbrev.is_empty() {
        number
    } else {
        format!("{number} {abbrev}")
    };
    Ok(FormattedQuantity {
        text,
        value: display.value,
        unit: display.unit,
    })
}
