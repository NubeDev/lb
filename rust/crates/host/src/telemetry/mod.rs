//! The **telemetry** host service (telemetry-console scope) — the gated, workspace-walled **read**
//! surface over the capped `telemetry` ring. It owns no writes (the `SurrealCappedLayer` is the only
//! writer); it owns the read verbs one responsibility per file (FILE-LAYOUT §3):
//!   - `authorize` — the `mcp:telemetry.<verb>:call` gate (workspace-first).
//!   - `filter`    — the query-filter → SurrealQL WHERE codec (source/actor/level/outcome/
//!     trace_id/text/time), all HARD-appended with `ws = caller`.
//!   - `query`     — `telemetry.query` (snapshot, filtered + seq-cursor paged, newest-first).
//!   - `trace`     — `telemetry.trace` (one correlated trace by `trace_id`, ws-filtered).
//!   - `tail`      — `telemetry.tail` (snapshot catch-up + the live ws-walled bus subscription).
//!   - `purge`     — `telemetry.purge` (node-admin destructive op).
//!   - `tool`      — the `telemetry.*` MCP bridge dispatch.
//!
//! **The headline boundary (the load-bearing wall):** the operator's raw ring legitimately spans
//! workspaces, but THIS tenant-facing view **hard-filters to the caller's `ws`** server-side — a
//! ws-B caller gets zero ws-A rows, regardless of the filter. That is the subtle seam the emission
//! scope flagged; it is enforced and tested HERE. A separate, higher node-admin capability governs
//! any cross-tenant operator read (not the default workspace grant).

mod authorize;
mod error;
mod filter;
mod purge;
mod query;
mod seed;
mod tail;
mod tool;
mod trace;

pub use authorize::{authorize_telemetry, read_or_admin_cap};
pub use error::TelemetrySvcError;
pub use filter::{QueryFilter, QueryPage};
pub use purge::telemetry_purge;
pub use query::{telemetry_query, TelemetryRow};
pub use seed::telemetry_seed;
pub use tail::{telemetry_tail, TailSnapshot, TailSub};
pub use tool::call_telemetry_tool;
pub use trace::telemetry_trace;

pub use lb_telemetry::TABLE as TELEMETRY_TABLE;
