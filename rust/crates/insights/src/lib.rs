//! `lb-insights` — the durable data-insight record (insights umbrella scope +
//! occurrences/subscriptions/notify sub-scopes).
//!
//! An **insight** is a persisted, queryable data finding raised by a rule, a flow, or an agent.
//! It carries severity, provenance, dedup-keyed occurrence counting, and an
//! `open → acked → resolved` lifecycle. This crate owns the **record shapes** + **pure verbs**
//! over the store seam, mirroring `lb-inbox`'s altitude (one verb per file, one responsibility
//! per file, no auth here — authorization is the host's job, run *after* `caps::check`).
//!
//! The umbrella owns the parent record + the three producer doors' shared types. Three sub-scope
//! record families live alongside:
//!   - [`occurrence`] / [`occurrences`] / [`occ_append`] — the per-insight transaction ring
//!     (`insight-occurrences-scope.md`).
//!   - [`subscription`] / [`sub_create`]…[`sub_mute`] / [`match_subs`] / [`intent`] — channel
//!     subscriptions + the raise-time matcher (`insight-subscriptions-scope.md`).
//!   - [`notify_state`] / [`policy`] / [`ladder`] / [`digest`] — the adaptive digest ladder
//!     (`insight-notify-scope.md`).
//!
//! All timing types take an injected logical clock (no wall-clock in core — testing §3). The
//! matcher, the ladder state machine, and the digest reactor are PURE / UNIT-TESTABLE with zero
//! I/O; their bodies are scaffolding stubs (`todo!()`) — see
//! `docs/sessions/insights/insights-scaffold-session.md` for the punch-list.

mod ack;
mod error;
mod get;
mod intent;
mod ladder;
mod list;
mod match_subs;
mod notify_apply;
mod notify_state;
mod notify_store;
mod occ_append;
mod occurrence;
mod occurrences;
mod origin;
mod policy;
mod policy_get;
mod policy_set;
mod raise;
mod resolve;
mod severity;
mod status;
mod sub_create;
mod sub_delete;
mod sub_get;
mod sub_list;
mod sub_mute;
mod subscription;
mod table_scan;
mod watch;

mod digest;
mod insight;
mod insight_id;

pub use ack::ack;
pub use error::InsightsError;
pub use get::get;
pub use intent::{Intent, IntentKind};
pub use ladder::{ladder_step, Delivery, DeliveryReason, LadderInput, Level, WindowAccumulator};
pub use list::{list, ListFilter, ListPage, ListQuery, PageCursor};
pub use match_subs::{match_subs, InsightView};
pub use notify_apply::apply_intents;
pub use notify_state::NotifyState;
pub use notify_store::{all_notify, notify_id, read_notify, write_notify};
pub use occ_append::{append_occurrence, validate_occurrence_size};
pub use occurrence::Occurrence;
pub use occurrences::{occurrences, OccCursor, OccurrencePage};
pub use origin::{Origin, OriginKind};
pub use policy::{defaults as policy_defaults, Policy, ThrottleOverride};
pub use policy_get::policy_get;
pub use policy_set::policy_set;
pub use raise::{raise, read_insight, RaiseInput, RaiseOutcome};
pub use resolve::resolve;
pub use severity::Severity;
pub use status::Status;
pub use sub_create::{sub_create, CreateInput as SubCreateInput};
pub use sub_delete::sub_delete;
pub use sub_get::sub_get;
pub use sub_list::sub_list;
pub use sub_mute::sub_mute;
pub use subscription::{DormantReason, SubFilter, SubSink, SubSinkKind, Subscription};
pub use watch::{event_subject, EventKind, RaiseEvent};

// Re-exports of the record shapes + table-const helpers (the host service + tests reach them
// through the crate root, exactly like `lb_inbox::record_id` / `lb_inbox::TABLE`).
pub use digest::{compute_due_digests, scan_notify_rows, DigestPass, PendingDigest, NOTIFY_TABLE};
pub use insight::{Insight, OCC_TABLE as INSIGHT_TABLE};
pub use policy::TABLE as POLICY_TABLE;
pub use subscription::TABLE as SUB_TABLE;
pub use insight_id::{dedup_lookup, record_id};
