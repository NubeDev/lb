//! **The escalation email** — the outbox effect `case.escalate` stages, one per recipient
//! (case-escalate scope).
//!
//! One responsibility: a case + the sender's words + one address → the `Effect` the existing email
//! `Target` already delivers. Same convention as the request link (`request_link.rs`):
//!
//! - **`workspace` is set explicitly** — the tenancy wall; `email_payload.rs` fails the effect
//!   permanently without it, and the catalog's `{workspace}` argument (the deep link) reads it.
//! - **The words come from the CATALOG**, keyed by the action: `case_escalate.email.subject` /
//!   `.body` / `.body_html`. Adding this mail added no match arm in `email_content.rs`.
//! - **No token.** The recipient is a workspace member who signs in; the link opens the case in the
//!   app, it does not authenticate anybody.
//!
//! **Idempotency is (escalation, recipient).** The escalation is keyed by the case and the caller's
//! logical `ts`, so a retried call with the same `ts` re-stages the SAME effect ids and the email
//! target's per-recipient delivered ledger stops a second send; each recipient gets their own effect
//! so one bounced address never re-sends to the others.

use lb_outbox::Effect;
use serde_json::json;

use super::request_link::link_args;
use crate::outbox::EMAIL_TARGET;

/// The outbox action for an escalation. Doubles as the catalog key prefix.
pub const ESCALATE_ACTION: &str = "case_escalate";

/// The effect id for the mail to `recipient` about the escalation of `case_id` at `ts`. The address
/// is folded to a SHA-256 prefix: an effect id is a store key and shows up in outbox listings, and
/// neither is a place an address belongs.
pub(super) fn escalate_effect_id(case_id: &str, ts: u64, recipient: &str) -> String {
    let digest = super::request_token::hash_request_token(recipient);
    format!("case-escalate:{case_id}:{ts}:{}", &digest[..16])
}

/// The words: the sender's subject and message (each with its `has_*` switch, HTML-escaped in the
/// `*_html` twin), the case's title and site, who escalated it, and the case id for the link.
pub(super) fn escalate_args(
    case: &lb_cases::Case,
    subject: Option<&str>,
    message: Option<&str>,
    sender: Option<&str>,
) -> serde_json::Value {
    let brief = lb_cases::Brief {
        subject: subject.map(str::to_string),
        message: message.map(str::to_string),
        sender: sender.map(str::to_string),
        ..Default::default()
    };
    let mut args = link_args(Some(&brief), case);
    if let Some(map) = args.as_object_mut() {
        map.insert("case_id".into(), json!(case.id));
    }
    args
}

/// Build the escalation email effect for one `recipient`.
pub(super) fn escalate_effect(
    case_id: &str,
    ws: &str,
    recipient: &str,
    args: &serde_json::Value,
    ts: u64,
) -> Effect {
    let id = escalate_effect_id(case_id, ts, recipient);
    let payload = json!({
        "recipients": [recipient],
        "workspace": ws,
        "locale": serde_json::Value::Null,
        "args": args,
    });
    Effect::new(
        &id,
        EMAIL_TARGET,
        ESCALATE_ACTION,
        payload.to_string(),
        &id,
        ts,
    )
}
