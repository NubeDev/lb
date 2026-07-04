//! `lb-frame` — polars-in-the-cage, the `Frame` surface of the rules data stdlib
//! (`docs/scope/rules/data-stdlib-scope.md`).
//!
//! A NEW crate (not folded into `lb-rules`) so the cage crate's "links only rhai + serde" doctrine
//! stays legible: the one heavy dependency (polars) is bounded here and severable behind `lb-rules`'
//! `frames` feature. This crate performs ZERO I/O: a `Frame` is post-collect, in-memory compute over
//! rows a [`DataSeam`] already gated (the rule author never sees a `scan_csv`/`read_parquet`/cloud
//! reader — they are not registered as SQL table functions on the pinned feature set, proven by the
//! `sql_security_test` probe; scope Non-goal).
//!
//! **Phase 0 (this crate):** compiles + a trivial polars `DataFrame` round-trips, proving the pinned
//! feature set builds clean through the zig-cc toolchain and the catalog's planned
//! `f.col("value") → plain array` + `frame(records)` shape is sound. The full folder-of-verbs
//! (`construct.rs`/`filter.rs`/`group.rs`/`window.rs`/`sql.rs`/`export.rs`) and the rhai
//! `register(engine, &FrameLimits)` entry point land in Phase 2; until then the JSON↔Frame boundary
//! + the input governors are the surface a future `verbs/frame.rs` reaches through.
//!
//! [`DataSeam`]: ../../rules/src/seam/trait.DataSeam.html

#![allow(dead_code)] // Phase 0: the boundary + limits are exercised by tests; the rhai surface lands in Phase 2.

mod json;
mod limits;

pub use json::{any_value_to_json, frame_col_json, frame_from_json, frame_to_json};
pub use limits::FrameLimits;
