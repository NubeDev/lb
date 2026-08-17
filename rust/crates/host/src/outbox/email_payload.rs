//! The **email effect payload** — the one place the opaque `Effect.payload` string becomes typed.
//!
//! The outbox payload is deliberately an opaque string per target (rule 10: the router knows nothing
//! about what any target's rows contain). This file is where the `email` target, and only the `email`
//! target, gives that string a shape.
//!
//! Two producers write it today and neither should have to know about the other:
//!   - **invites** (`invites/create.rs`) send `{email, workspace, token, locale}` and let the prefs
//!     catalog supply the words;
//!   - **a scheduled report** (the rubix-ai renderer) sends `{workspace, recipients[], subject, body,
//!     assetId}` — the words are authored by whoever wrote the schedule, and the artefact is an asset
//!     reference.
//!
//! So both an `email` string and a `recipients` array are accepted, and both a catalog-rendered and a
//! payload-authored body. What is NOT optional is `workspace`: it is the tenancy wall, it selects the
//! credential and the asset namespace, and an absent one fails the effect rather than defaulting
//! (rule 6). That refusal is the single most important line in this file.

use super::delivery_error::DeliveryError;

/// The typed view of an email effect's payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct EmailPayload {
    /// Every address this effect goes to, in payload order, de-duplicated. Never empty.
    pub recipients: Vec<String>,
    /// The tenancy wall. Never defaulted.
    pub workspace: String,
    /// The locale the catalog renders in (`""` ⇒ the resolver's `en` fallback).
    pub locale: String,
    /// The invite token, for the catalog's `{token}` argument. Empty for anything else.
    pub token: String,
    /// A payload-authored subject, when the producer wrote one.
    pub subject: Option<String>,
    /// A payload-authored plain-text body.
    pub body: Option<String>,
    /// A payload-authored HTML body.
    pub html: Option<String>,
}

impl EmailPayload {
    /// Parse `raw` (the effect's payload string).
    ///
    /// Every failure here is **permanent**: malformed JSON does not become well-formed on the fifth
    /// retry, and neither does a payload that names nobody.
    pub(super) fn parse(raw: &str) -> Result<(Self, serde_json::Value), DeliveryError> {
        let json: serde_json::Value = serde_json::from_str(raw).map_err(|e| {
            DeliveryError::permanent(format!("email target: bad payload json: {e}"))
        })?;

        let recipients = recipients(&json);
        if recipients.is_empty() {
            return Err(DeliveryError::permanent(
                "email target: payload names no recipient (`email` or `recipients`)".to_string(),
            ));
        }

        let workspace = field(&json, "workspace").ok_or_else(|| {
            DeliveryError::permanent(
                "email target: payload missing workspace — refusing to guess (rule 6)".to_string(),
            )
        })?;

        let payload = Self {
            recipients,
            workspace,
            locale: field(&json, "locale").unwrap_or_default(),
            token: field(&json, "token").unwrap_or_default(),
            subject: field(&json, "subject"),
            body: field(&json, "body"),
            html: field(&json, "html"),
        };
        // The raw JSON travels on for the attachment resolver, which reads its own keys. Handing it
        // back rather than re-parsing keeps one parse per delivery.
        Ok((payload, json))
    }
}

/// The addresses, from either carrier, in order, without duplicates.
///
/// The de-duplication is not cosmetic: each recipient gets its own delivered-ledger row and its own
/// send, so a payload that listed an address twice would put two copies in one inbox.
fn recipients(json: &serde_json::Value) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut push = |address: String| {
        if !out.contains(&address) {
            out.push(address);
        }
    };
    if let Some(one) = field(json, "email") {
        push(one);
    }
    for row in json
        .get("recipients")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
    {
        if let Some(address) = row.as_str().map(str::trim).filter(|a| !a.is_empty()) {
            push(address.to_string());
        }
    }
    out
}

/// A non-blank string field, or `None`.
fn field(json: &serde_json::Value, key: &str) -> Option<String> {
    json.get(key)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_invite_payload_parses_through_the_single_email_carrier() {
        let (p, _) = EmailPayload::parse(
            &serde_json::json!({
                "email": "sam@example.com",
                "workspace": "nube",
                "token": "lbi_abc",
                "locale": "fr",
            })
            .to_string(),
        )
        .unwrap();
        assert_eq!(p.recipients, vec!["sam@example.com"]);
        assert_eq!(p.workspace, "nube");
        assert_eq!(p.token, "lbi_abc");
        assert_eq!(p.locale, "fr");
        assert_eq!(p.subject, None);
    }

    #[test]
    fn a_report_payload_parses_through_the_recipients_array_with_authored_words() {
        let (p, raw) = EmailPayload::parse(
            &serde_json::json!({
                "workspace": "nube",
                "recipients": ["ap@nube-io.com", "ops@nube-io.com"],
                "subject": "energy — 2026-08-10 → 2026-08-17",
                "body": "The weekly energy report is attached.",
                "assetId": "report-energy-week",
            })
            .to_string(),
        )
        .unwrap();
        assert_eq!(p.recipients, vec!["ap@nube-io.com", "ops@nube-io.com"]);
        assert_eq!(
            p.subject.as_deref(),
            Some("energy — 2026-08-10 → 2026-08-17")
        );
        assert_eq!(
            p.body.as_deref(),
            Some("The weekly energy report is attached.")
        );
        // The raw JSON is handed back so the attachment resolver reads its keys from one parse.
        assert_eq!(raw.get("assetId").unwrap(), "report-energy-week");
    }

    #[test]
    fn a_repeated_address_is_sent_to_once() {
        let (p, _) = EmailPayload::parse(
            &serde_json::json!({
                "workspace": "nube",
                "email": "ap@nube-io.com",
                "recipients": ["ap@nube-io.com", " ", "ops@nube-io.com"],
            })
            .to_string(),
        )
        .unwrap();
        assert_eq!(p.recipients, vec!["ap@nube-io.com", "ops@nube-io.com"]);
    }

    #[test]
    fn a_payload_without_a_workspace_is_refused_permanently() {
        let err =
            EmailPayload::parse(&serde_json::json!({ "email": "sam@example.com" }).to_string())
                .expect_err("the tenancy wall must not be defaulted");
        assert!(err.permanent, "{err}");
        assert!(err.reason.contains("workspace"), "{err}");
    }

    #[test]
    fn a_payload_with_no_recipient_is_refused_rather_than_delivered_nowhere() {
        let err = EmailPayload::parse(
            &serde_json::json!({ "workspace": "nube", "recipients": [] }).to_string(),
        )
        .expect_err("an effect that names nobody cannot be delivered");
        assert!(err.permanent, "{err}");
        assert!(err.reason.contains("recipient"), "{err}");
    }

    #[test]
    fn payload_that_is_not_json_fails_permanently() {
        let err = EmailPayload::parse("not json").expect_err("must refuse");
        assert!(err.permanent, "{err}");
    }
}
