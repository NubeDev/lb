//! `to_display(value, from_unit, dimension, resolved)` — convert a canonical value into the unit a
//! user's resolved prefs say to *show* it in (prefs scope `format.quantity`). The `from_unit` is
//! the value's source unit (the `unit:` tag for ingest, the producer's declaration otherwise); the
//! `to` unit is `resolved.display_unit(dimension)` (override → unit_system default).
//!
//! Returns the converted magnitude + the target `Unit` (the caller renders the number with `icu`
//! and appends the unit's display abbreviation). A `from_unit` whose dimension disagrees with the
//! requested `dimension` is a hard error — the provenance must be truthful.

use crate::axis::{Dimension, Unit};
use crate::error::PrefsError;
use crate::prefs::ResolvedPrefs;

use super::unit_convert::convert;

/// The result of resolving a canonical quantity to its display form: the magnitude in `unit`.
pub struct DisplayQuantity {
    pub value: f64,
    pub unit: Unit,
}

/// Convert `value` (in `from_unit`) to the display unit `resolved` chooses for `dimension`. Errors
/// if `from_unit` is not of `dimension` (mismatched provenance) — never a silent reinterpretation.
pub fn to_display(
    value: f64,
    from_unit: Unit,
    dimension: Dimension,
    resolved: &ResolvedPrefs,
) -> Result<DisplayQuantity, PrefsError> {
    if from_unit.dimension() != dimension {
        return Err(PrefsError::CrossDimension {
            from: from_unit.as_str(),
            from_dim: from_unit.dimension().as_str(),
            to: dimension.as_str(),
            to_dim: dimension.as_str(),
        });
    }
    let target = resolved.display_unit(dimension);
    let value = convert(value, from_unit, target)?;
    Ok(DisplayQuantity {
        value,
        unit: target,
    })
}
