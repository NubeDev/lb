//! The **insights** service — the capability-gated surface over `lb_insights` (insights umbrella
//! scope + occurrences/subscriptions/notify sub-scopes). The durable records + the pure verbs
//! already exist in `lb_insights`; this layer gates them and host-stamps the un-spoofable fields
//! (`producer`, `owner`, `acked_by`, the `Origin.kind` the caller cannot choose).
//!
//! Authorization is the MCP gate (`mcp:insight.<verb>:call`) through `authorize_tool` (workspace-
//! first §7, then capability §3.5). The raw record persistence stays in `lb_insights`; this layer
//! is authorization + author-forcing only (one verb per file, FILE-LAYOUT §3).
//!
//! Verbs:
//!   - `insight.raise` ([`insight_raise`]) — the producer WRITE (dedup on `(ws, dedup_key)`).
//!   - `insight.get` ([`insight_get`]) — read one insight by id.
//!   - `insight.list` ([`insight_list`]) — faceted, keyset-paged newest-first.
//!   - `insight.ack` ([`insight_ack`]) — `open → acked` (status_by host-forced).
//!   - `insight.resolve` ([`insight_resolve`]) — `* → resolved` (idempotent).
//!   - `insight.occurrences` ([`insight_occurrences`]) — read the per-insight occurrence ring.
//!   - `insight.sub.{create,list,get,delete,mute}` — channel subscriptions (subscriptions scope).
//!   - `insight.policy.{get,set}` — the workspace policy record (notify scope).
//!   - the MCP bridge ([`call_insight_tool`]) — the one MCP contract over all of the above.
//!
//! The `insight.watch` SSE surface + the digest reactor are wired in their own files (the
//! reactor follows the flows/reminders owner-election precedent; both are stubbed).

mod ack;
mod error;
mod get;
mod list;
mod occurrences;
mod policy_get;
mod policy_set;
mod raise;
mod resolve;
mod sub_create;
mod sub_delete;
mod sub_get;
mod sub_list;
mod sub_mute;
mod tool;

pub use ack::insight_ack;
pub use error::InsightSvcError;
pub use get::insight_get;
pub use list::insight_list;
pub use occurrences::insight_occurrences;
pub use policy_get::insight_policy_get;
pub use policy_set::insight_policy_set;
pub use raise::insight_raise;
pub use resolve::insight_resolve;
pub use sub_create::insight_sub_create;
pub use sub_delete::insight_sub_delete;
pub use sub_get::insight_sub_get;
pub use sub_list::insight_sub_list;
pub use sub_mute::insight_sub_mute;
pub use tool::call_insight_tool;
