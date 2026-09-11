//! `case.request.withdraw` — take an ask back (case-plane scope, wave 2).
//!
//! Gated on `mcp:case.request.send:call` through a `tool_gate.rs` alias: withdrawing is the same
//! authority as asking. A separate `mcp:case.request.withdraw:call` would exist in no role bundle
//! and be `Denied` for every caller including admins — the shipped-but-unusable trap that table
//! exists to prevent — and nobody grants "may ask a contractor to quote but may never take it
//! back".
//!
//! Three things happen, and the order matters: the row is marked `withdrawn` (which also pulls
//! `expires_ts` back to now, so the token dies with the ask rather than staying live until the
//! original window closed), the nudge ladder is cancelled, and a `request_withdrawn` event is
//! appended. A withdrawal that left the ladder running would chase a contractor about a job we
//! cancelled.
//!
//! A **replied** request cannot be withdrawn — the crate refuses it. The answer already happened,
//! and pretending otherwise would erase the one record of what the party told us.

use lb_auth::Principal;
use lb_cases::{CaseRequest, EventKind};
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::CaseSvcError;
use super::request_nudge_schedule::cancel_nudges;

/// Withdraw request `id` in `ws` as `principal` at logical `ts`. Idempotent: withdrawing an
/// already-withdrawn ask is a no-op that appends no second event.
pub async fn case_request_withdraw(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    ts: u64,
) -> Result<CaseRequest, CaseSvcError> {
    authorize_tool(principal, ws, "case.request.send").map_err(|_| CaseSvcError::Denied)?;

    let Some(mut request) = lb_cases::request_get(store, ws, id).await? else {
        return Err(CaseSvcError::BadInput(format!("no such request: {id}")));
    };

    if lb_cases::request_withdraw(store, ws, &mut request, ts).await? {
        cancel_nudges(store, ws, &request.id).await;
        lb_cases::append_event(
            store,
            ws,
            &request.case_id,
            EventKind::RequestWithdrawn,
            principal.sub(),
            serde_json::json!({ "request_id": request.id, "party_id": request.party_id }),
            ts,
        )
        .await?;
    }
    Ok(request)
}
