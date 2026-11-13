//! `case.escalate` — hand a case to a workspace member or team AND email them about it, in one step
//! (case-escalate scope).
//!
//! The internal twin of `case.request.send`. That verb mails an EXTERNAL party (the roster) a
//! tokenised reply link, schedules nudges and sets `waiting_on`. This one picks from the same
//! subjects `case.assign` accepts (`user:…` / `team:…`), because the person on the other end signs
//! in: no token, no nudge ladder, no `waiting_on` — the case is now theirs, and the mail is a link
//! to it.
//!
//! **Gate: `mcp:case.request.send:call`** (aliased in `tool_gate.rs`). "May email people about a
//! case" is exactly the grant this needs; minting `mcp:case.escalate:call` would put a cap in no
//! role bundle and make the verb `Denied` for everyone. The assign half additionally needs
//! `case.workflow` — checked up front here (and again inside `case_assign`), so a caller with the
//! mail grant but not the triage one is refused before anything is written. Strictest wins.
//!
//! What one call does, in order — every refusal happens before the first write:
//!   1. resolve the case (an out-of-scope case reads exactly like a missing one);
//!   2. validate `to` through `validate_assignee` (the same opacity contract as `case.assign`);
//!   3. resolve the recipients' addresses server-side ([`super::escalate_recipients`]) — none ⇒
//!      refused, and the case is NOT reassigned;
//!   4. size-check the history row it will append;
//!   5. assign through `case_assign` (so the owner echo onto the member insights happens);
//!   6. stage one email effect per recipient through the outbox;
//!   7. append an `escalated` event: who, to whom, how many mails, and the words — never an address.

use std::sync::Arc;

use lb_auth::Principal;
use lb_cases::{Case, CaseEvent, EventKind};
use lb_mcp::authorize_tool;

use super::error::CaseSvcError;
use super::escalate_mail::{escalate_args, escalate_effect};
use super::escalate_recipients::escalation_recipients;
use crate::boot::Node;
use crate::insight::validate_assignee;

/// The table each staged mail's justification row lands in (`lb_outbox::enqueue` writes a change
/// and its effect in one transaction; an escalation has no record of its own besides the event).
const ESCALATION_TABLE: &str = "case_escalation";

/// What the verb hands back: the case after the assign, and how many mails were staged. No
/// addresses — see the module note in `escalate_recipients.rs`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Escalation {
    pub case: Case,
    pub recipients: usize,
}

/// Escalate case `case_id` to `to` as `principal`: assign it and email the subject's people.
#[allow(clippy::too_many_arguments)]
pub async fn case_escalate(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    case_id: &str,
    to: &str,
    subject: Option<&str>,
    message: Option<&str>,
    ts: u64,
) -> Result<Escalation, CaseSvcError> {
    authorize_tool(principal, ws, "case.request.send").map_err(|_| CaseSvcError::Denied)?;
    authorize_tool(principal, ws, "case.workflow").map_err(|_| CaseSvcError::Denied)?;

    let store = &node.store;
    let missing = || CaseSvcError::BadInput(format!("no such case: {case_id}"));
    let Some(case) = lb_cases::get(store, ws, case_id).await? else {
        return Err(missing());
    };
    if !super::entity_filter::visible(store, principal, ws, &case).await? {
        return Err(missing());
    }
    validate_assignee(store, ws, to)
        .await
        .map_err(|e| CaseSvcError::BadInput(e.to_string()))?;

    let recipients = escalation_recipients(store, ws, to).await?;
    if recipients.is_empty() {
        return Err(CaseSvcError::BadInput(format!("no email address for {to}")));
    }

    let subject = words(subject);
    let message = words(message);
    let data = serde_json::json!({
        "to": to,
        "recipients": recipients.len(),
        "subject": subject,
        "message": message,
    });
    // The history row is checked BEFORE the assign and the mails, so an oversize message refuses the
    // whole escalation rather than leaving a reassigned case with no record of why.
    lb_cases::validate_event_size(&CaseEvent {
        seq: 0,
        ts,
        kind: EventKind::Escalated,
        actor: principal.sub().to_string(),
        data: data.clone(),
    })?;

    super::case_assign(node, principal, ws, case_id, Some(to), ts).await?;

    let sender = lb_authz::identity_get(store, principal.sub())
        .await
        .ok()
        .flatten()
        .and_then(|i| i.display_name);
    let args = escalate_args(&case, subject, message, sender.as_deref());
    for recipient in &recipients {
        let effect = escalate_effect(case_id, ws, recipient, &args, ts);
        let change = serde_json::json!({
            "case_id": case_id,
            "to": to,
            "by": principal.sub(),
            "ts": ts,
        });
        lb_outbox::enqueue(store, ws, ESCALATION_TABLE, &effect.id, &change, &effect)
            .await
            .map_err(|e| CaseSvcError::Store(format!("stage the escalation mail: {e}")))?;
    }

    lb_cases::append_event(
        store,
        ws,
        case_id,
        EventKind::Escalated,
        principal.sub(),
        data,
        ts,
    )
    .await?;

    let case = lb_cases::get(store, ws, case_id).await?.unwrap_or(case);
    Ok(Escalation {
        case,
        recipients: recipients.len(),
    })
}

/// The sender's text, trimmed; blank is absent.
fn words(text: Option<&str>) -> Option<&str> {
    text.map(str::trim).filter(|t| !t.is_empty())
}
