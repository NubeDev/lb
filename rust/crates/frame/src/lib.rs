//! `lb-frame` — polars-in-the-cage, the `Frame` surface of the rules data stdlib
//! (`docs/scope/rules/data-stdlib-scope.md`).
//!
//! A NEW crate (not folded into `lb-rules`) so the cage crate's "links only rhai + serde" doctrine
//! stays legible: the one heavy dependency (polars) is bounded here and severable behind `lb-rules`'
//! `frames` feature. This crate performs ZERO I/O: a `Frame` is post-collect, in-memory compute over
//! rows a `DataSeam` already gated (the rule author never sees a `scan_csv`/`read_parquet`/cloud
//! reader — they are not registered as SQL table functions on the pinned feature set, proven by the
//! `sql_security_test` probe; scope Non-goal).
//!
//! **Phase 2 (this build):** the full folder-of-verbs rhai surface behind ONE entry point,
//! [`register`]. Every frame-producing verb's OUTPUT passes the [`FrameLimits`] row/cell caps —
//! the rhai deadline cannot interrupt a native polars call, so the bound lives on the values
//! themselves — and every export path normalizes NaN/Inf → null (the scope's NaN/null policy).

mod aggregate;
mod construct;
mod convert;
mod export;
mod filter;
mod group;
mod inspect;
mod json;
mod limits;
mod missing;
mod series;
mod shape;
mod sql;
mod timebucket;
mod value;

pub use construct::frame_from_grid;
pub use json::{any_value_to_json, frame_col_json, frame_from_json, frame_to_json};
pub use limits::FrameLimits;
pub use value::Frame;

/// The ONE entry point (the scope's contract): wire the `Frame` value type, the `frame(records)`
/// constructor, and every Frame method into a rhai engine, all closed over the run's limits.
/// `g.frame()` (Grid materialization) is registered by `lb-rules` — the gated seam lives there.
pub fn register(engine: &mut rhai::Engine, limits: &FrameLimits) {
    engine.register_type_with_name::<Frame>("Frame");
    construct::register(engine, limits);
    inspect::register(engine);
    shape::register(engine);
    filter::register(engine);
    missing::register(engine);
    aggregate::register(engine);
    group::register(engine);
    series::register(engine);
    timebucket::register(engine);
    sql::register(engine);
    export::register(engine);
}
