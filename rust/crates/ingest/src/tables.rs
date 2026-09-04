//! The ingest plane's table names.
//!
//! One place, so a reader never has to guess which string a query means. The rollup, registry,
//! newest-pointer and retention tables are named by their own modules (`schema`, `meta`,
//! `retention`) — this file holds the two the commit transaction writes.

/// The committed time-series table. A sample record lives at `series:[series, producer, seq]`.
pub const SERIES_TABLE: &str = "series";

/// The dead-letter table for samples the series cardinality cap refused — diverted, never silently
/// dropped (ingest scope).
pub const DEAD_LETTER_TABLE: &str = "ingest_dead_letter";
