//! The flows slice's shared table-drain. The cursor loop itself lives ONE layer down in
//! [`lb_store::scan_all`] (the canonical, cross-crate seam — every host service that reads a whole ws
//! table goes through it; debugging/flows/single-scan-page-drops-rows-past-200.md). Re-exported here so
//! the flows verbs keep their `super::scan_all::scan_all` import path.
//!
//! Why a full drain (not a prefix early-exit) and why no silent backstop: see `lb_store::scan_all`'s
//! docs. Short form — the scan cursor is the SurrealDB `<string>id` rendering whose ordering disagrees
//! with the display id, so a prefix-seeded early exit is unsound; and a partial return would just
//! relocate the "rows vanish past N" bug. The flows tables are retention-bounded (`retain_runs`,
//! sweeps), so the full drain stays small.

pub use lb_store::scan_all;
