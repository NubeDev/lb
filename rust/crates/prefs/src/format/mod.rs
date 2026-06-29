//! Presentation rendering — canonical value in, localized string out (prefs scope). `datetime`
//! applies a timezone over a UTC instant (DST-correct via `chrono-tz`); `number` applies the
//! locale's separators; `quantity` composes uom conversion + number rendering for the chart bridge.
//! All pure — the same code runs on edge and cloud, fully offline.

mod datetime;
mod number;
mod quantity;

pub use datetime::format_datetime;
pub use number::{format_number, NumberOpts};
pub use quantity::{format_quantity, FormattedQuantity};
