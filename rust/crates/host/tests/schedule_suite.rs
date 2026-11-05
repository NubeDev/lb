//! Aggregated **global schedule** integration tests: the `schedule.*` store surface over a real
//! `mem://` store, no mocks. One binary for the suite (the `case_suite.rs` pattern); the files live in
//! `schedule/`, which Cargo does not auto-discover as targets.

#[path = "schedule/support.rs"]
mod support;

#[path = "schedule/read_rev.rs"]
mod read_rev;

#[path = "schedule/evaluate.rs"]
mod evaluate;

#[path = "schedule/save_rev.rs"]
mod save_rev;

#[path = "schedule/save_shape.rs"]
mod save_shape;

#[path = "schedule/flow_node.rs"]
mod flow_node;
