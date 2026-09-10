//! The **case** service — the capability-gated surface over `lb_cases` (case-plane scope). The
//! durable records + the pure verbs already exist in `lb_cases`; this layer gates them, host-stamps
//! the un-spoofable fields (`actor` from the principal, never the input), normalizes the logical
//! clock, and owns the two reactors the crate is deliberately agnostic of.
//!
//! Authorization is the MCP gate through `authorize_tool` (workspace-first §7, then capability
//! §3.5). Four of the eleven verbs gate on a capability that is **not their own name** — see the
//! table below — and every one of those needs an arm in `tool_gate.rs`, because the outer gate
//! consults that table and a missing arm demands a cap no role bundle carries: `Denied` for every
//! caller, admins included, and only a POSITIVE test catches it.
//!
//! | verb | capability |
//! |---|---|
//! | `case.get`, `case.members`, `case.events` | `case.get` (viewer) |
//! | `case.list` | `case.list` (viewer) |
//! | `case.open`, `case.merge`, `case.split` | `case.open` (member) |
//! | `case.workflow`, `case.assign`, `case.snooze`, `case.comment` | `case.workflow` (member) |
//! | `policy.sla.set` | `policy.sla.set` (**admin**) |
//! | `policy.sla.list` | `policy.sla.list` (**admin**) |
//! | `rule.scorecard` | `rule.scorecard` (viewer) |
//!
//! The two reactors:
//!   - **case-group** ([`group_insight`]) — runs INLINE at the end of `insight_raise` and from the
//!     reconcile loop, so "every open insight is in exactly one open case" is true immediately
//!     rather than eventually.
//!   - **hold-down** ([`hold_down::reopen_if_held`]) — reached through `group_insight`, so the raise
//!     path has one call site and not two.
//!
//! [`reconcile_cases`] is the restart-safe backstop and the triage backfill;
//! [`spawn_case_reactors`] is its loop driver.

mod assign;
mod cites;
mod clock;
mod comment;
mod echo;
mod error;
mod events;
mod facets;
mod get;
mod group;
mod hold_down;
mod list;
mod members;
mod merge;
mod open;
mod policy_list;
mod policy_set;
mod reactor;
mod reconcile;
mod scorecard;
mod snooze;
mod split;
mod tool;
mod verdict;
mod workflow;

pub use assign::case_assign;
// The owner echo: the case is the authority for who owns the work, and every insight it cites
// carries a copy so the roster's owner column stays a one-read render. `insight.assign` delegates
// here (case-plane scope, resolved decision 5).
pub use assign::echo_owner_to_members as echo_case_owner;
pub use comment::case_comment;
pub use error::CaseSvcError;
pub use events::case_events;
pub use get::case_get;
pub use list::case_list;
pub use members::case_members;
pub use merge::case_merge;
pub use open::case_open;
pub use policy_list::case_policy_sla_list;
pub use policy_set::case_policy_sla_set;
pub use scorecard::{rule_scorecard, UNKNOWN_RULE_REF};
pub use snooze::case_snooze;
pub use split::case_split;
pub use tool::call_case_tool;
pub use workflow::case_workflow;

pub use group::{group_insight, GROUP_ACTOR};
pub use hold_down::reopen_if_held;
pub use reactor::spawn_case_reactors;
pub use reconcile::reconcile_cases;
