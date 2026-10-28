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
    args: serde_json::Value,
    locale: Option<&str>,
    ts: u64,
) -> Effect {
    let id = link_effect_id(request_id);
    let payload = json!({
        "recipients": [recipient],
        "workspace": ws,
        "token": token,
        "locale": locale,
        "args": args,
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
    reference: Option<&str>,
    locale: Option<&str>,
    ts: u64,
) -> Effect {
    let id = nudge_effect_id(request_id, stage);
    let payload = json!({
        "recipients": [recipient],
        "workspace": ws,
        "locale": locale,
        "args": reference_args(reference),
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

/// The catalog arguments for the sender's reference: the text (empty when there is none, so a
/// workspace's own wording can use a plain `{reference}`), and `has_reference` (`yes` / `no`) for the
/// built-in copy's `select`, so a mail with no reference reads as it always did.
/// The link email's words: the sender's name for the work, where it is, what was found, who
/// flagged it, and their reference — each with its `has_*` switch so the copy reads well without it.
/// `title`/`site` fall back to the case's own.
pub(super) fn link_args(
    brief: Option<&lb_cases::Brief>,
    case: &lb_cases::Case,
) -> serde_json::Value {
    let pick = |v: Option<&String>| v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let title = pick(brief.and_then(|b| b.title.as_ref())).unwrap_or_else(|| case.title.clone());
    let site = pick(brief.and_then(|b| b.site.as_ref())).or_else(|| pick(case.site.as_ref()));
    let mut args = reference_args(brief.and_then(|b| b.reference.as_deref()));
    let map = args.as_object_mut().expect("reference_args is an object");
    map.insert("title".into(), json!(title));
    for (key, value) in [
        ("site", site),
        ("finding", pick(brief.and_then(|b| b.finding.as_ref()))),
        ("sender", pick(brief.and_then(|b| b.sender.as_ref()))),
    ] {
        map.insert(key.into(), json!(value.clone().unwrap_or_default()));
        map.insert(
            format!("has_{key}"),
            json!(if value.is_some() { "yes" } else { "no" }),
        );
    }
    args
}

fn reference_args(reference: Option<&str>) -> serde_json::Value {
    match reference.map(str::trim).filter(|r| !r.is_empty()) {
        Some(r) => json!({ "reference": r, "has_reference": "yes" }),
        None => json!({ "reference": "", "has_reference": "no" }),
    }
}
