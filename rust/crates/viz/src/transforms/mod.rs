//! The transformer registry (viz transformations scope, FILE-LAYOUT: one transformer per file). Each
//! submodule owns exactly one Grafana transformer id; the dispatcher in `transform.rs` calls its
//! `apply(frames, options)`. A folder-of-verbs, never a `transforms.rs` of nouns.

pub mod calculate_field;
pub mod filter_by_name;
pub mod filter_by_value;
pub mod group_by;
pub mod join_by_field;
pub mod limit;
pub mod merge;
pub mod organize;
pub mod reduce;
pub mod series_to_rows;
pub mod sort_by;
