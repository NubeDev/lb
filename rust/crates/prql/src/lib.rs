//! `lb-prql` — the PRQL authoring layer (query scope). A thin, **pure** wrapper over the official
//! `prqlc` compiler: one entry point ([`compile`]) that turns PRQL text into a dialect-specific SQL
//! string. **Zero I/O** — no filesystem, no network, no store; the same code runs on edge and cloud,
//! fully offline. The host `query` service calls this, then hands the SQL to the engine that already
//! owns the wall (`store.query` for platform, `federation.query` for a datasource).
//!
//! One responsibility per file (FILE-LAYOUT §3):
//!   - [`dialect`] — the [`Dialect`] we expose (the subset the query surface targets) + the map to
//!     `prqlc`'s dialect enum, picked from a target / datasource kind.
//!   - [`compile`] — the single `compile(prql, dialect) -> sql` function.
//!   - [`error`]  — the typed error ([`PrqlError`]).

pub mod compile;
pub mod dialect;
pub mod error;

pub use compile::compile;
pub use dialect::{dialect_for_kind, dialect_for_target, Dialect};
pub use error::PrqlError;

/// The pinned `prqlc` version this crate wraps (golden tests freeze against it; a bump is a reviewed
/// change with re-frozen goldens).
pub const PRQLC_VERSION: &str = "0.13";
