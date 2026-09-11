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
//! | `case.request.send` | `case.request.send` (member) |
//! | `case.request.withdraw`, `case.request.nudge` | `case.request.send` (member) |
//! | `case.request.list` | `case.get` (viewer) |
//! | `case.request.view`, `case.request.reply` | **their own caps, in NO bundle** — token-only |
//! | `party.upsert` | `party.upsert` (**admin**) |
//! | `party.list` | `party.list` (**admin**) |
//! | `case.breach` | `case.breach` (**no role bundle** — see [`breach`]) |
//! | `rule.scorecard` | `rule.scorecard` (viewer) |
//!
//! The two reactors:
//!   - **case-group** ([`group_insight`]) — runs INLINE at the end of `insight_raise` and from the
//!     reconcile loop, so "every open insight is in exactly one open case" is true immediately
//!     rather than eventually.
//!   - **hold-down** ([`hold_down::reopen_if_held`]) — reached through `group_insight`, so the raise
//!     path has one call site and not two.
//!   - **sla-clock** ([`apply_sla`]) — runs on the tail of `group_insight` and after `case_open`,
//!     so one code path resolves the contract at open AND on a severity escalation. It arms a
//!     durable one-shot reminder that fires [`case_breach`] at `due_at`; nothing pauses that clock.
//!
//! [`reconcile_cases`] is the restart-safe backstop and the triage backfill;
//! [`spawn_case_reactors`] is its loop driver.

mod assign;
mod breach;
mod breach_notify;
mod breach_reminder;
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
mod party_list;
mod party_upsert;
mod policy_list;
mod policy_set;
mod reactor;
mod reconcile;
mod request_attach;
mod request_authenticate;
mod request_delivery;
mod request_link;
mod request_list;
mod request_nudge;
mod request_nudge_schedule;
mod request_reply_verb;
mod request_scope;
mod request_send;
mod request_token;
mod request_view;
mod request_window;
mod request_withdraw;
mod scorecard;
mod sla_clock;
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
pub use party_list::case_party_list;
pub use party_upsert::{case_party_upsert, PartyInput};
pub use policy_list::case_policy_sla_list;
pub use policy_set::case_policy_sla_set;
pub use scorecard::{rule_scorecard, UNKNOWN_RULE_REF};
// The external-party round trip (wave 2). `send`/`withdraw`/`nudge` are AUTHOR verbs; `view` and
// `reply` are the ONLY two a token principal may call, and both re-check that the token is scoped
// to the one request id (`request_scope.rs` — `Principal::constraint` is a cap set and cannot
// narrow to a record, so the narrowing lives inside the verbs).
pub use request_attach::{case_request_attach, AttachmentReceipt};
pub use request_authenticate::{case_request_authenticate, RequestTokenError};
pub use request_list::case_request_list;
pub use request_nudge::case_request_nudge;
pub use request_reply_verb::{case_request_reply, ReplyReceipt};
pub use request_scope::{PARTY_SUB_PREFIX, REPLY_CAP, VIEW_CAP};
pub use request_send::case_request_send;
pub use request_token::{hash_request_token, workspace_of_token};
pub use request_view::{case_request_view, RequestView};
pub use request_withdraw::case_request_withdraw;
pub use snooze::case_snooze;
pub use split::case_split;
pub use tool::call_case_tool;
pub use workflow::case_workflow;

/// The MCP tool a nudge reminder fires. Named here, once, because the scheduler writes it into a
/// durable reminder row and the dispatcher must answer to the same string years later.
pub(crate) const NUDGE_TOOL: &str = "case.request.nudge";

pub use group::{group_insight, GROUP_ACTOR};
pub use hold_down::reopen_if_held;
pub use reactor::spawn_case_reactors;
pub use reconcile::reconcile_cases;
