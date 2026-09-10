//! `lb-cases` — the durable **case** record: a piece of work that cites insights
//! (`docs/scope/insights/case-plane-scope.md`).
//!
//! **An insight is a detection. A case is a piece of work. They are different records.** An insight
//! is keyed by `(workspace, dedup_key)`, re-raised and re-opened by a machine, and deliberately
//! immune on its human plane (`insight-triage-scope.md`). Work is **many-to-one** with detections:
//! one gateway drop is 400 detections and one job; one chiller that short-cycles every summer is one
//! detection key and three jobs over three years. Putting workflow on the detection row breaks both.
//!
//! This crate owns the **record shapes** + **pure verbs** over the store seam, mirroring
//! `lb-insights`' altitude exactly: one verb per file, one responsibility per file, **no
//! authorization here** — that is the host's job (`host/src/case/`), run *after* `caps::check`. No
//! wall clock either: every `ts` is a caller-injected logical epoch-millis value (testing §3).
//!
//! **Rule 10 holds throughout.** Nothing here names a pack, a rule, a category *value* or an
//! extension. A case is "a record that cites insights"; `grouping: verdict` is decided by the host
//! reading `body.explains[]` — a property of the data, not a rule name.
//!
//! Three record families:
//!   - [`case`] — the case itself, its workflow, its money and its deadlines.
//!   - [`case_member`] — the citation edges, and the **exclusivity invariant** ([`member_add`]):
//!     every open insight is in exactly ONE open case.
//!   - [`case_event`] — the append-only history that never evicts ([`event_append`]).
//!
//! The two invariants worth knowing before editing anything here:
//!   1. **`resolved` requires a `resolution`, and nothing else may carry one** ([`workflow`]).
//!   2. **Every open insight is in exactly one open case** ([`member_add`] refuses to break it;
//!      [`merge`] / [`split`] are the verbs that legitimately move members).

mod assign;
mod calendar;
mod case;
mod case_event;
mod case_member;
mod comment;
mod deadline;
mod error;
mod event_append;
mod events;
mod find_by_insight;
mod get;
mod list;
mod member_add;
mod member_remove;
mod members;
mod merge;
mod open;
mod policy;
mod policy_list;
mod policy_match;
mod policy_set;
mod save;
mod snooze;
mod split;
mod workflow;

pub use assign::{assign, AssignOutcome};
pub use calendar::{Calendar, CalendarError, DayHours, MINUTES_PER_DAY};
pub use case::{
    severity_rank, Case, Grouping, ImpactTier, Resolution, WaitingOn, Workflow, TABLE as CASE_TABLE,
};
pub use case_event::{
    validate_event_size, CaseEvent, EventKind, MAX_EVENT_DATA_BYTES, TABLE as CASE_EVENT_TABLE,
};
pub use case_member::{member_id, CaseMember, MemberRole, TABLE as CASE_MEMBER_TABLE};
pub use comment::comment;
pub use deadline::{add_business_hours, due_at, respond_by, MAX_DAYS_SCANNED, NEVER};
pub use error::CasesError;
pub use event_append::append_event;
pub use events::{events, EventPage, MAX_EVENT_PAGE};
pub use find_by_insight::{
    find_open_case_for_insight, last_closed_case_for_insight, memberships_of_insight,
};
pub use get::get;
pub use list::{CaseRow, Lane, ListFilter, ListPage, ListQuery, MAX_CASE_PAGE};
pub use member_add::member_add;
pub use member_remove::member_remove;
pub use members::{member_count, members, MemberPage, MAX_MEMBER_PAGE};
pub use merge::merge;
pub use open::{open, OpenInput};
pub use policy::{
    PolicyMatch, ServicePolicy, DEFAULT_HOLD_DOWN_DAYS, DEFAULT_PARTY_WINDOW_H, POLICY_TABLE,
};
pub use policy_list::{policy_list, sort_by_specificity};
pub use policy_match::{best_match, match_policy};
pub use policy_set::policy_set;
pub use snooze::{puncture_snooze, snooze};
pub use split::split;
pub use workflow::workflow;

// The lane read is a whole-table scan behind one name, so callers never reach for `lb_store::list`
// with a hand-built filter and re-derive the `{ data, rev }` unwrap.
pub use list::list;
