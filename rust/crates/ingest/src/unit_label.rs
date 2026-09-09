//! Source-unit provenance at commit — a producer's `unit` label becomes the series' registered
//! `unit` (`series_meta.unit`), so a value read back can be converted into a viewer's display unit.
//!
//! # Why a label, and why this one
//!
//! `lb_prefs` ships a correct uom-backed converter, but it needs a `from_unit` and nothing on the
//! data path carried one. A producer already declares dimensions in `Sample.labels` (`{host:
//! "pi-7"}`), so a `unit` label is the seam that already exists — no new wire field, no new verb,
//! and a producer that declares nothing keeps working unchanged.
//!
//! The label is **validated against the closed [`Unit`] vocabulary and dropped when it does not
//! parse**. That refusal is the point: free text like `"degrees celsius"` looks like provenance and
//! converts to nothing, which is strictly worse than an absent unit (the caller renders the
//! canonical value and says so). A unit that lb cannot convert must not be recorded as one it can.
//!
//! Unlike the label→tag conversion this has **no once-per-series latch**. A series' unit can
//! legitimately change (a rescaled sensor, a corrected declaration), and the registry holds one
//! current value rather than a history — so the newest declaration wins. The write is skipped when
//! the stored unit already matches, so the steady state costs one read, not one write, per series.

use lb_prefs::axis::Unit;
use lb_store::{Store, StoreError};

use crate::meta::{set_unit, unit as stored_unit};
use crate::sample::Sample;

/// The label key a producer declares its source unit under.
pub const UNIT_LABEL: &str = "unit";

/// The unit `sample.labels` declares, if it declares one lb can actually convert. `None` for an
/// absent label, a non-string value, or a token outside the closed vocabulary.
pub fn declared_unit(sample: &Sample) -> Option<Unit> {
    sample
        .labels
        .as_object()?
        .get(UNIT_LABEL)?
        .as_str()
        .and_then(Unit::parse)
}

/// Register `sample`'s declared source unit against its series, if it declares a parseable one.
/// A no-op when nothing is declared, and when the registry already holds that same unit.
pub async fn apply_unit(store: &Store, ws: &str, sample: &Sample) -> Result<(), StoreError> {
    let Some(declared) = declared_unit(sample) else {
        return Ok(());
    };
    if stored_unit(store, ws, &sample.series).await? == Some(declared) {
        return Ok(());
    }
    set_unit(store, ws, &sample.series, declared).await
}
