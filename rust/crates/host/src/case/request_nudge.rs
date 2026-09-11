//! `case.request.nudge` — one rung of the ladder, fired by a reminder (case-plane scope, wave 2).
//!
//! Gated on `mcp:case.request.send:call` through a `tool_gate.rs` alias: chasing a party about an
//! ask is the same authority as making it. It is reached only by the reminder reactor, under the
//! **sender's stored principal with caps re-resolved at fire time** — so revoking the sender's
//! grant stops the ladder, which is the property a nudge test must turn off to prove it is testing
//! anything at all (`green-while-broken-reactor-tests.md`).
//!
//! **The two rungs are not the same act.**
//!   - `n50` / `n80` **nudge the party**: another email, no link (the raw token is unrecoverable by
//!     construction and minting a second one would kill the link already in their inbox — see the
//!     catalog note beside `request_nudge.email.body`).
//!   - `breach` **escalates inward**: the window closed. There is no point mailing someone who has
//!     already ignored two reminders, so this rung appends a `breach` event on the case and leaves
//!     `waiting_on` exactly where it is — the case is still blocked on them, and that is precisely
//!     what the breach report needs to say.
//!
//! **A nudge on an answered ask is a no-op**, not an error: the ladder is cancelled on reply and on
//! withdrawal, and this is the second line of defence for the race where the reminder fires in the
//! same tick as the reply lands.

use lb_auth::Principal;
use lb_cases::EventKind;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::CaseSvcError;
use super::request_link::nudge_effect;
use super::request_nudge_schedule::cancel_nudges;

/// The ladder rung that escalates instead of chasing.
const BREACH_STAGE: &str = "breach";

/// Fire the `stage` rung for request `id`. Returns whether anything was actually sent or recorded.
pub async fn case_request_nudge(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    stage: &str,
    ts: u64,
) -> Result<bool, CaseSvcError> {
    authorize_tool(principal, ws, "case.request.send").map_err(|_| CaseSvcError::Denied)?;

    let Some(mut request) = lb_cases::request_get(store, ws, id).await? else {
        return Err(CaseSvcError::BadInput(format!("no such request: {id}")));
    };

    // Answered or withdrawn ⇒ nothing to chase. Cancel whatever is left of the ladder so the
    // remaining rungs do not each re-discover this.
    if !request.status.is_live() {
        cancel_nudges(store, ws, &request.id).await;
        return Ok(false);
    }

    if stage == BREACH_STAGE {
        lb_cases::request_expire_if_due(store, ws, &mut request, ts).await?;
        lb_cases::append_event(
            store,
            ws,
            &request.case_id,
            EventKind::Breach,
            principal.sub(),
            serde_json::json!({
                "request_id": request.id,
                "party_id": request.party_id,
                "respond_by": request.respond_by,
                "nudges_sent": request.nudges_sent,
            }),
            ts,
        )
        .await?;
        return Ok(true);
    }

    // A chasing rung whose window has already closed chases nobody: the breach rung is the one
    // that speaks after the deadline, and a "please reply" arriving after "you missed it" is the
    // platform contradicting itself. (Reachable when a rung fires late — after an outage, say.)
    if lb_cases::is_expired(&request, ts) {
        return Ok(false);
    }

    // A chasing rung: stage another mail, per-stage idempotent, and count it.
    let Some(party) = lb_cases::party_get(store, ws, &request.party_id).await? else {
        return Ok(false);
    };
    let Some(email) = party.contact.email.as_deref() else {
        return Ok(false);
    };

    let effect = nudge_effect(&request.id, stage, ws, email, None, ts);
    let change = serde_json::json!({
        "request_id": request.id,
        "stage": stage,
        "by": principal.sub(),
        "ts": ts,
    });
    lb_outbox::enqueue(
        store,
        ws,
        lb_cases::CASE_REQUEST_TABLE,
        &format!("{}-nudge-{stage}", request.id),
        &change,
        &effect,
    )
    .await
    .map_err(|e| CaseSvcError::Store(format!("stage the nudge: {e}")))?;

    let sent = lb_cases::request_bump_nudges(store, ws, &mut request).await?;
    lb_cases::append_event(
        store,
        ws,
        &request.case_id,
        EventKind::Nudge,
        principal.sub(),
        serde_json::json!({
            "request_id": request.id,
            "party_id": request.party_id,
            "stage": stage,
            "nudges_sent": sent,
        }),
        ts,
    )
    .await?;
    Ok(true)
}
