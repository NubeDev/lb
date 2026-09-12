//! Aggregated **Case plane** integration tests.
//!
//! Cargo compiles every top-level `tests/*.rs` as its own crate, statically linking the whole
//! dependency graph (SurrealDB, Zenoh, wasmtime) into each one — ~1 GB per binary. Declaring the
//! case-plane suites as modules of ONE harness keeps the file layout intact (one responsibility per
//! file, `docs/FILE-LAYOUT.md`) while producing a single binary, exactly as `agent_suite.rs` does
//! for the 29 agent files.
//!
//! Each suite is a `*_support.rs` fixture module plus the focused files that use it. The `#[path]`
//! attributes mean `case/` is just a directory Cargo does not auto-discover as targets.

#[path = "case/delegation_assign.rs"]
mod delegation_assign;
#[path = "case/delegation_bulk.rs"]
mod delegation_bulk;
#[path = "case/delegation_notes.rs"]
mod delegation_notes;
#[path = "case/delegation_support.rs"]
mod delegation_support;

#[path = "case/sla_policy_gates.rs"]
mod sla_policy_gates;
#[path = "case/sla_policy_ladder.rs"]
mod sla_policy_ladder;
#[path = "case/sla_policy_support.rs"]
mod sla_policy_support;
#[path = "case/sla_policy_walls.rs"]
mod sla_policy_walls;

#[path = "case/caveat_support.rs"]
mod caveat_support;

#[path = "case/caveat_echo.rs"]
mod caveat_echo;

#[path = "case/caveat_stamp.rs"]
mod caveat_stamp;

#[path = "case/caveat_vocab.rs"]
mod caveat_vocab;

#[path = "case/caveat_vocab_door.rs"]
mod caveat_vocab_door;

#[path = "case/scorecard_support.rs"]
mod scorecard_support;

#[path = "case/scorecard_filters.rs"]
mod scorecard_filters;

#[path = "case/scorecard_gates.rs"]
mod scorecard_gates;

#[path = "case/scorecard_join.rs"]
mod scorecard_join;

#[path = "case/scorecard_walls.rs"]
mod scorecard_walls;

#[path = "case/sla_clock_support.rs"]
mod sla_clock_support;

#[path = "case/sla_clock_alarm.rs"]
mod sla_clock_alarm;

#[path = "case/sla_clock_breach.rs"]
mod sla_clock_breach;

#[path = "case/sla_clock_calendar.rs"]
mod sla_clock_calendar;

#[path = "case/reactor_support.rs"]
mod reactor_support;

#[path = "case/reactor_grouping.rs"]
mod reactor_grouping;

#[path = "case/reactor_hold_down.rs"]
mod reactor_hold_down;

#[path = "case/reactor_reconcile.rs"]
mod reactor_reconcile;

#[path = "case/reactor_verdict.rs"]
mod reactor_verdict;

#[path = "case/plane_support.rs"]
mod plane_support;

#[path = "case/plane_immunity.rs"]
mod plane_immunity;

#[path = "case/plane_invariants.rs"]
mod plane_invariants;

#[path = "case/plane_lanes.rs"]
mod plane_lanes;

#[path = "case/plane_members.rs"]
mod plane_members;

#[path = "case/plane_snooze.rs"]
mod plane_snooze;

#[path = "case/plane_split.rs"]
mod plane_split;

#[path = "case/plane_walls.rs"]
mod plane_walls;

#[path = "case/request_support.rs"]
mod request_support;

#[path = "case/request_attachment.rs"]
mod request_attachment;

#[path = "case/request_delivery.rs"]
mod request_delivery;

#[path = "case/request_nudge.rs"]
mod request_nudge;

#[path = "case/request_roster.rs"]
mod request_roster;

#[path = "case/request_send.rs"]
mod request_send;

#[path = "case/request_token.rs"]
mod request_token;

#[path = "case/request_walls.rs"]
mod request_walls;
