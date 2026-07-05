//! `ack` — move an insight `open → acked` (insights umbrella scope).
//!
//! Records who acked and when (logical ts). An acked fault re-firing bumps `count` but does NOT
//! re-page anyone (the ladder's ack-suppression, notify scope). `status_by` is host-stamped from
//! the principal — never caller-supplied (a caller cannot forge another reviewer's ack). No-op if
//! already `acked`; refused (returns `BadInput`) if `resolved` (a resolved insight stays resolved
//! — re-open is the raise verb's job, not ack's).
//!
//! **STUB**: the state-transition + refuse-on-resolved body is deferred — see the punch-list.

use lb_store::Store;

use crate::error::InsightsError;

/// Ack insight `id` in workspace `ws` as `acked_by` at logical ts `ts`. `acked_by` is the
/// principal's `sub` (host-supplied, never caller-supplied).
// SCOPE: docs/scope/insights/insights-scope.md §"MCP surface" (insight.ack)
// SCOPE: docs/scope/insights/insight-notify-scope.md §"Ack means 'I know'" (the suppression rule)
pub async fn ack(
    _store: &Store,
    _ws: &str,
    _id: &str,
    _acked_by: &str,
    _ts: u64,
) -> Result<(), InsightsError> {
    // 1. Read the insight; if absent ⇒ BadInput ("no such insight").
    // 2. If `resolved` ⇒ BadInput ("resolved insights stay resolved — re-open via raise").
    // 3. If already `acked` ⇒ no-op success (idempotent).
    // 4. Else set status=acked, status_by=acked_by, status_ts=ts; write back.
    todo!("insights: ack transition (refuse on resolved, idempotent on acked) — SCOPE: insights-scope.md §MCP surface")
}
