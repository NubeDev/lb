//! The transformer registry (viz transformations scope, FILE-LAYOUT: one transformer per file). Each
//! submodule owns exactly one Grafana transformer id; the dispatcher in `transform.rs` calls its
//! `apply(frames, options)`. A folder-of-verbs, never a `transforms.rs` of nouns.

pub mod calculate_field;
pub mod concatenate;
pub mod convert_field_type;
pub mod extract_fields;
pub mod filter_by_name;
pub mod filter_by_ref_id;
pub mod filter_by_value;
pub mod group_by;
pub mod join_by_field;
pub mod labels_to_fields;
pub mod limit;
pub mod merge;
pub mod organize;
pub mod reduce;
pub mod rename_by_regex;
pub mod series_to_rows;
pub mod sort_by;
