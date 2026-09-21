//! Aggregated **members** integration tests — the team-membership verbs and the picker they feed.
//!
//! Cargo compiles every top-level `tests/*.rs` as its own crate, statically linking the whole
//! dependency graph into each one. Declaring these suites as modules of ONE harness keeps the file
//! layout intact (one responsibility per file, `docs/FILE-LAYOUT.md`) while producing a single
//! binary, exactly as `case_suite.rs` does.

#[path = "members/support.rs"]
mod support;

#[path = "members/bridge.rs"]
mod bridge;

#[path = "members/picker.rs"]
mod picker;

#[path = "members/spelling.rs"]
mod spelling;
