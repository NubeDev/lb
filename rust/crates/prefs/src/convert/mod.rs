//! Unit conversion — **uom-backed correctness** (prefs scope). `unit_convert` is the raw
//! same-dimension convert (`convert.unit` verb); `quantity` resolves a canonical value to a user's
//! display unit (`format.quantity`). Pure: no store, no auth, no clock.

mod quantity;
mod unit_convert;

pub use quantity::{to_display, DisplayQuantity};
pub use unit_convert::convert;
