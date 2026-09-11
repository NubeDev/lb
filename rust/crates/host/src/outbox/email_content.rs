//! **The words** — how an email effect's action becomes a subject, a text body, and an HTML body.
//!
//! Two sources, in a fixed precedence:
//!
//! 1. **The payload, when the producer authored words.** A schedule whose author typed
//!    "Weekly energy — site B" must get exactly that in the subject line. Nothing may translate,
//!    re-word, or override it.
//! 2. **The `lb_prefs` catalog, in the effect's locale**, keyed by the action. This is how invites work
//!    and how any future action gets multi-language copy for free: ship
//!    `{prefix}.email.subject` / `.body` / `.body_html` keys and the arm below needs no edit.
//!
//! The catalog lookup is **derived from the action string**, not a match arm per action — that is the
//! point. Adding an emailed action is a catalog change, not a code change here. The one exception is
//! the legacy `send_invite` action, whose catalog keys were named `invite.email.*` before this
//! convention existed; that single mapping is spelled out rather than renaming shipped catalog keys
//! (which would break every translation already written against them).
//!
//! The last fallback is honest rather than blank: an action with no catalog copy and no authored words
//! gets the action name as its subject, so a mis-wired producer produces a legible mail an operator can
//! trace, not an empty one.

use std::collections::BTreeMap;

use super::email_payload::EmailPayload;

/// Subject + both body halves, ready for a provider.
pub(super) struct EmailContent {
    pub subject: String,
    pub text: String,
    pub html: Option<String>,
}

/// Render the content for `action` from `payload` and the catalog.
///
/// `base_url` is the node's public origin, supplied by the target from deployment config. It becomes
/// the catalog's `{base_url}` argument, so a copy key writes `{base_url}/r/{token}` and gets an
/// ABSOLUTE link. When the node has not been told its public origin the argument renders empty and
/// the link stays relative — the behaviour every emailed link had before, and still legible to a
/// human reading the mail even though a client cannot resolve it.
pub(super) fn content_for(
    action: &str,
    payload: &EmailPayload,
    base_url: Option<&str>,
) -> EmailContent {
    let resolved = lb_prefs::resolve(&[lb_prefs::Prefs {
        language: Some(payload.locale.clone()),
        ..Default::default()
    }]);
    let args = serde_json::json!({
        "workspace": payload.workspace,
        "token": payload.token,
        "base_url": base_url.unwrap_or_default(),
    });
    let empty = BTreeMap::new();
    let prefix = catalog_prefix(action);
    // `render_message` never returns blank — an absent key renders as the key literal, its documented
    // last fallback. `catalog` turns that back into `None` so the payload/derived fallbacks can win.
    let catalog = |leaf: &str| {
        let key = format!("{prefix}.email.{leaf}");
        let rendered = lb_prefs::render_message(&key, &args, &empty, &resolved).text;
        let trimmed = rendered.trim();
        if trimmed.is_empty() || trimmed == key {
            None
        } else {
            Some(rendered)
        }
    };

    EmailContent {
        subject: payload
            .subject
            .clone()
            .or_else(|| catalog("subject"))
            .unwrap_or_else(|| default_subject(action)),
        text: payload
            .body
            .clone()
            .or_else(|| catalog("body"))
            .unwrap_or_default(),
        html: payload.html.clone().or_else(|| catalog("body_html")),
    }
}

/// The catalog key prefix for `action`.
///
/// The convention is "the action IS the prefix". `send_invite` predates the convention and its keys
/// ship as `invite.email.*` in every translated catalog, so it maps rather than being renamed.
fn catalog_prefix(action: &str) -> &str {
    match action {
        "send_invite" => "invite",
        other => other,
    }
}

/// The subject an action with no copy anywhere gets: the action name, humanised. Legible and
/// traceable — the previous behaviour was the constant "Notification", which told a recipient (and an
/// operator reading a mailbox) nothing at all about which producer sent it.
fn default_subject(action: &str) -> String {
    let mut chars = action.replace(['_', '-', '.'], " ");
    if let Some(first) = chars.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    chars
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload(subject: Option<&str>, body: Option<&str>) -> EmailPayload {
        EmailPayload {
            recipients: vec!["a@b.c".into()],
            workspace: "nube".into(),
            locale: String::new(),
            token: "lbi_abc123".into(),
            subject: subject.map(str::to_string),
            body: body.map(str::to_string),
            html: None,
        }
    }

    #[test]
    fn the_invite_action_still_renders_from_the_shipped_catalog_keys() {
        let c = content_for("send_invite", &payload(None, None), None);
        assert!(c.text.contains("lbi_abc123"), "{}", c.text);
        assert!(!c.subject.is_empty());
        assert_ne!(
            c.subject, "Send invite",
            "the catalog must win over the fallback"
        );
        let html = c.html.expect("the catalog ships an HTML alternative");
        assert!(html.contains("lbi_abc123"), "{html}");
    }

    #[test]
    fn authored_words_win_over_the_catalog() {
        let c = content_for(
            "send_invite",
            &payload(Some("Join us"), Some("hello")),
            None,
        );
        assert_eq!(c.subject, "Join us");
        assert_eq!(c.text, "hello");
    }

    #[test]
    fn an_action_with_no_copy_gets_a_traceable_subject_not_a_blank_one() {
        let c = content_for("report", &payload(None, None), None);
        assert_eq!(c.subject, "Report");
        assert_eq!(c.text, "");
        assert_eq!(c.html, None);
    }

    #[test]
    fn a_report_effect_carries_its_authored_subject_and_body() {
        let c = content_for(
            "report",
            &payload(Some("energy — week 33"), Some("Attached.")),
            None,
        );
        assert_eq!(c.subject, "energy — week 33");
        assert_eq!(c.text, "Attached.");
    }

    #[test]
    fn a_public_origin_makes_the_request_link_absolute() {
        // The whole point of the field: a contractor gets this in a mail client, which has no origin
        // to resolve a relative path against.
        let mut p = payload(None, None);
        p.token = "lbr_nube.SECRET".into();
        let c = content_for("request_link", &p, Some("https://insights.example.com"));
        assert!(
            c.text
                .contains("https://insights.example.com/r/lbr_nube.SECRET"),
            "text body must carry an absolute link: {}",
            c.text
        );
        let html = c.html.expect("the catalog ships an HTML alternative");
        assert!(
            html.contains("href=\"https://insights.example.com/r/lbr_nube.SECRET\""),
            "html href must be absolute: {html}"
        );
    }

    #[test]
    fn without_a_public_origin_the_link_stays_relative_and_still_carries_the_token() {
        // The RED half of the case above, and the documented pre-existing behaviour: a node that was
        // never told its public name renders `{base_url}` empty rather than the literal `{base_url}`.
        // The link is unusable in a mail client — that is the limitation, honestly rendered — but the
        // token is still legible to a human reading the mail.
        let mut p = payload(None, None);
        p.token = "lbr_nube.SECRET".into();
        let c = content_for("request_link", &p, None);
        assert!(
            c.text.contains("/r/lbr_nube.SECRET"),
            "the token must survive: {}",
            c.text
        );
        assert!(
            !c.text.contains("{base_url}"),
            "an unset origin must render EMPTY, never the placeholder literal: {}",
            c.text
        );
        assert!(
            !c.text.contains("//r/"),
            "an empty origin must not produce a protocol-relative `//r/`: {}",
            c.text
        );
    }
}
