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
mod breach_mark;
mod calendar;
mod case;
mod case_event;
mod case_member;
mod case_request;
mod comment;
mod deadline;
mod deadline_set;
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
mod party;
mod party_list;
mod party_upsert;
mod policy;
mod policy_list;
mod policy_match;
mod policy_set;
mod reply_transition;
mod request_delivery;
mod request_get;
mod request_open;
mod request_reply;
mod request_save;
mod request_status;
mod resolved;
mod save;
mod scorecard;
mod snooze;
mod split;
mod waiting_on;
mod workflow;

pub use assign::{assign, AssignOutcome};
pub use breach_mark::mark_breached;
pub use calendar::{Calendar, CalendarError, DayHours, MINUTES_PER_DAY};
pub use case::{
    severity_rank, Case, Grouping, ImpactTier, Resolution, WaitingOn, Workflow, TABLE as CASE_TABLE,
};
pub use case_event::{
    validate_event_size, CaseEvent, EventKind, MAX_EVENT_DATA_BYTES, TABLE as CASE_EVENT_TABLE,
};
pub use case_member::{member_id, CaseMember, MemberRole, TABLE as CASE_MEMBER_TABLE};
// The external-party round trip (wave 2): the ask, the token hash, the reply. `Delivery` is what
// happened to the MAIL and `RequestStatus` is where the ASK is — never one field (resolved
// decision 7).
pub use case_request::{
    validate_reply, Ask, Brief, BriefEvidence, BriefSeries, CaseRequest, Delivery, Reply,
    ReplyKind, RequestStatus, MAX_REPLY_ATTACHMENTS, MAX_REPLY_TEXT_BYTES,
    TABLE as CASE_REQUEST_TABLE,
};
pub use comment::comment;
pub use deadline::{add_business_hours, due_at, respond_by, MAX_DAYS_SCANNED, NEVER};
pub use deadline_set::{set_deadlines, Deadlines};
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
// The party roster (wave 2). A party is DATA: nothing here names one (rule 10), and the scorecard
// is derived by query over `case_request`, never stored on the party.
pub use party::{
    validate_party, Contact, Party, PartyKind, MAX_PARTY_NAME_BYTES, TABLE as PARTY_TABLE,
};
pub use party_list::{party_get, party_list};
pub use party_upsert::party_upsert;
pub use policy::{
    PolicyMatch, ServicePolicy, DEFAULT_HOLD_DOWN_DAYS, DEFAULT_PARTY_WINDOW_H, POLICY_TABLE,
};
pub use policy_list::{policy_list, sort_by_specificity};
pub use policy_match::{best_match, match_policy};
pub use policy_set::policy_set;
pub use reply_transition::transition_for;
pub use request_delivery::{request_bump_nudges, request_set_delivery};
pub use request_get::{request_by_token_hash, request_get, requests_of_case};
pub use request_open::{new_request, request_open, RequestInput};
pub use request_reply::{request_reply, ReplyOutcome};
pub use request_status::{
    is_expired, request_expire_if_due, request_mark_opened, request_withdraw,
};
pub use resolved::{resolved_cases, ResolvedFilter};
pub use scorecard::{scorecard, ResolvedCase, ScorecardRow};
pub use snooze::{puncture_snooze, snooze};
pub use split::split;
pub use waiting_on::{set_cost_to_fix, set_waiting_on};
pub use workflow::workflow;

// The lane read is a whole-table scan behind one name, so callers never reach for `lb_store::list`
// with a hand-built filter and re-derive the `{ data, rev }` unwrap.
pub use list::list;
