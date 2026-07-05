//! `resolve` — move an insight `* → resolved` (insights umbrella scope).
//!
//! Idempotent (re-resolving a resolved insight is a no-op). Records who resolved and when. An
//! optional `note` rides the resolution (a human closure reason). `status_by` is host-stamped
//! from the principal — never caller-supplied. After resolve, a subsequent raise **re-opens**
//! (the raise verb's branch — this verb only closes).
//!
//! **STUB**: the transition body is deferred — see the punch-list.

use lb_store::Store;

use crate::error::InsightsError;

/// Resolve insight `id` in workspace `ws` as `resolved_by` at logical ts `ts`, with an optional
/// `note`. Idempotent on an already-resolved insight.
// SCOPE: docs/scope/insights/insights-scope.md §"MCP surface" (insight.resolve)
pub async fn resolve(
    _store: &Store,
    _ws: &str,
    _id: &str,
    _resolved_by: &str,
    _note: Option<&str>,
    _ts: u64,
) -> Result<(), InsightsError> {
    // 1. Read the insight; if absent ⇒ BadInput ("no such insight").
    // 2. If already `resolved` ⇒ no-op success (idempotent).
    // 3. Else set status=resolved, status_by=resolved_by, status_ts=ts; attach `note` to the
    //    record's body (under a `resolution` key) if provided; write back.
    todo!("insights: resolve transition (idempotent) — SCOPE: insights-scope.md §MCP surface")
}
