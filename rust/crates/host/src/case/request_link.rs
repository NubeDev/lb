//! **The link email** — the outbox effect a `case.request.send` stages (case-plane scope, wave 2).
//!
//! One responsibility: turn a request + a party + a raw token into the `Effect` the existing email
//! `Target` already knows how to deliver. Nothing here knows about SMTP, Postmark, or any provider
//! (rule 10) — `target: "email"` is an opaque routing string and the adapter behind it is chosen at
//! boot.
//!
//! Three details are load-bearing:
//!
//! - **`workspace` is set explicitly, never defaulted.** It is the tenancy wall, it selects the
//!   credential and the asset namespace, and `email_payload.rs` fails the effect permanently
//!   without it. Passing it is the single most important line in the payload.
//! - **The words come from the CATALOG, keyed by the action.** `request_link.email.subject` /
//!   `.body` / `.body_html` ship in `en.mf`; `email_content.rs` derives the prefix from the action
//!   string, so this file adds no match arm anywhere. That is the whole convention: an emailed
//!   action is a catalog change.
//! - **Idempotency is the request id.** One ask, one effect, whatever the relay does — a re-enqueue
//!   of the same id is a no-op and a re-delivery dedups per recipient in the ledger.

use lb_outbox::Effect;
use serde_json::json;

use crate::outbox::EMAIL_TARGET;

/// The outbox action for the link email. Doubles as the catalog key prefix (see the module note).
pub const LINK_ACTION: &str = "request_link";

/// The outbox action for a nudge. Same convention, different words.
pub const NUDGE_ACTION: &str = "request_nudge";

/// The effect id for request `request_id`'s link mail — also its idempotency key.
pub(super) fn link_effect_id(request_id: &str) -> String {
    format!("case-request:{request_id}")
}

/// The effect id for the `n`-th nudge on `request_id`. Per-stage, so the three ladder mails are
/// three effects and a replayed nudge job re-stages the SAME one rather than mailing twice.
pub(super) fn nudge_effect_id(request_id: &str, stage: &str) -> String {
    format!("case-request-nudge:{request_id}:{stage}")
}

/// Build the link email effect. `token` is the RAW token and appears only here, on its way into the
/// mail — it is never stored (only its hash is, on the request row).
pub(super) fn link_effect(
    request_id: &str,
    ws: &str,
    recipient: &str,
    token: &str,
    locale: Option<&str>,
    ts: u64,
) -> Effect {
    let id = link_effect_id(request_id);
    let payload = json!({
        "recipients": [recipient],
        "workspace": ws,
        "token": token,
        "locale": locale,
    });
    Effect::new(&id, EMAIL_TARGET, LINK_ACTION, payload.to_string(), &id, ts)
}

/// Build a nudge email effect. Carries no token — see the catalog note beside
/// `request_nudge.email.body`: the raw token is unrecoverable by construction and minting a second
/// one would kill the link already in the recipient's inbox.
pub(super) fn nudge_effect(
    request_id: &str,
    stage: &str,
    ws: &str,
    recipient: &str,
    locale: Option<&str>,
    ts: u64,
) -> Effect {
    let id = nudge_effect_id(request_id, stage);
    let payload = json!({
        "recipients": [recipient],
        "workspace": ws,
        "locale": locale,
    });
    Effect::new(
        &id,
        EMAIL_TARGET,
        NUDGE_ACTION,
        payload.to_string(),
        &id,
        ts,
    )
}
