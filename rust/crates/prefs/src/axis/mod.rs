//! The **closed axis set** — every preference dimension, each its own independently-overridable
//! enum (prefs scope, "decouple the axes"). One file per axis; the axis vocabulary is the part the
//! client shares verbatim (generated constants), so it lives apart from the record/resolution code.

pub mod date_style;
pub mod dimension;
pub mod first_day;
pub mod language;
pub mod number_format;
pub mod time_style;
pub mod unit;
pub mod unit_system;

pub use date_style::DateStyle;
pub use dimension::Dimension;
pub use first_day::FirstDay;
pub use number_format::NumberFormat;
pub use time_style::TimeStyle;
pub use unit::Unit;
pub use unit_system::UnitSystem;
