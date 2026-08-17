//! The **email outbox target** — an `impl Target` that delivers every email the platform sends.
//!
//! The provider is the one sanctioned external: a true external you cannot run locally (an SMTP relay /
//! an email API). It lives behind one trait ([`EmailProvider`]) and the impls live beside this file —
//! [`SmtpEmailProvider`](super::SmtpEmailProvider), [`PostmarkEmailProvider`](super::PostmarkEmailProvider),
//! and the logging/recording pair in `email_provider_dev.rs`.
//!
//! What this file owns is everything *between* the effect row and the wire:
//! - fan the effect out to each recipient it names, one message each,
//! - dedup **per recipient** against the delivered ledger so a retry after a partial failure does not
//!   re-send to whoever already got it,
//! - hand the provider a stable `Message-ID` so the *receiving* side can collapse a duplicate too.
//!
//! Its three collaborators each own one thing: `email_payload.rs` types the opaque payload string,
//! `email_content.rs` decides the words, `email_attachment.rs` turns an asset reference into bytes.
//!
//! It owns no transport: no socket, no credential, no provider name (rule 10 — the effect's `target`
//! string is opaque routing data and this file is reached only through it).

use async_trait::async_trait;
use lb_outbox::Effect;
use lb_store::Store;
use serde::Deserialize;

use super::delivered::{delivery_check, delivery_mark};
use super::delivery_error::DeliveryError;
use super::email_attachment::{resolve_attachments, EmailAttachment};
use super::email_content::content_for;
use super::email_payload::EmailPayload;
use crate::outbox::Target;

/// The outbox target string for email delivery.
pub const EMAIL_TARGET: &str = "email";

/// One outbound message, as the target hands it to a provider.
///
/// The HTML half is why this is a struct rather than the four loose `&str`s it used to be: an HTML mail
/// without a plain-text alternative scores badly with every spam filter and is unreadable in a text
/// client, so the pair travels together and the transport builds `multipart/alternative` from it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EmailMessage {
    /// The recipient address. Exactly one — an effect naming several is fanned out into several
    /// messages, so each has its own delivery outcome and its own ledger row.
    pub to: String,
    pub subject: String,
    /// The plain-text body. Always populated.
    pub text: String,
    /// The optional HTML body (from the catalog's `*_html` key, or authored in the payload).
    pub html: Option<String>,
    /// Files to hang off the message — a scheduled report's PDF, resolved from the asset it names.
    pub attachments: Vec<EmailAttachment>,
    /// A stable `Message-ID` for this effect (WITHOUT angle brackets) — identical across retries, so a
    /// receiving MTA can collapse a duplicate the outbox could not know it sent. A mitigation, not a
    /// guarantee: an MTA may ignore it (see `delivered.rs` for the window that remains).
    pub message_id: Option<String>,
}

/// The email provider — the one sanctioned external. A product host may wire its own impl through the
/// boot seam; the shipped impls are SMTP and Postmark, and the test impl records sends.
#[async_trait]
pub trait EmailProvider: Send + Sync {
    /// Send `message`. `meta` carries the workspace + action (opaque to the provider — it may log the
    /// workspace, never a credential or a token).
    ///
    /// The error type is the outbox's [`DeliveryError`], so a provider states whether its failure is
    /// worth retrying: a `4xx`/timeout is transient (the outbox backs off), a `5xx`/bad recipient is
    /// permanent (the effect is parked with the reason). A provider MUST sanitize its reason — the text
    /// is durable and operator-visible.
    async fn send(&self, message: &EmailMessage, meta: &EmailMeta) -> Result<(), DeliveryError>;
}

/// Metadata passed to the email provider alongside the message.
#[derive(Debug, Clone, Deserialize)]
pub struct EmailMeta {
    pub workspace: String,
    #[serde(default)]
    pub action: String,
}

/// A shared provider delivers like its inner provider — lets a test hold an
/// `Arc<RecordingEmailProvider>` for assertions while the `EmailTarget` (and the relay reactor
/// that owns it) holds a clone. Delivery to a real relay is otherwise unobservable.
#[async_trait]
impl<P: EmailProvider + ?Sized> EmailProvider for std::sync::Arc<P> {
    async fn send(&self, message: &EmailMessage, meta: &EmailMeta) -> Result<(), DeliveryError> {
        (**self).send(message, meta).await
    }
}

/// The email `Target` adapter — reads the effect payload, resolves the words and the files, calls the
/// provider once per recipient. Matches on `effect.target == "email"` (the [`EMAIL_TARGET`] const).
pub struct EmailTarget {
    provider: Box<dyn EmailProvider>,
    /// For the delivered ledger (retry dedup) and for reading an attachment's asset. The target holds a
    /// store handle for the same reason `PushTarget` does — the dedup marker is durable state, and
    /// losing it means re-sending.
    store: Store,
}

impl EmailTarget {
    pub fn new(provider: Box<dyn EmailProvider>, store: Store) -> Self {
        Self { provider, store }
    }
}

impl Target for EmailTarget {
    fn deliver(
        &self,
        effect: &Effect,
    ) -> impl std::future::Future<Output = Result<(), DeliveryError>> + Send {
        let raw = effect.payload.clone();
        let action = effect.action.clone();
        let effect_id = effect.id.clone();
        let idempotency_key = effect.idempotency_key.clone();
        let effect_ts = effect.ts;
        let provider = &self.provider;
        let store = self.store.clone();
        async move {
            let (payload, json) = EmailPayload::parse(&raw)?;

            // Retry-dedup key: the outbox's own idempotency handle (falls back to the effect id).
            let dedup_key = if idempotency_key.is_empty() {
                effect_id
            } else {
                idempotency_key
            };

            let content = content_for(&action, &payload);
            // Resolved ONCE for the whole fan-out: an effect with five recipients reads the PDF once,
            // not five times. It is resolved before any send so a missing asset fails the effect
            // rather than mailing the first recipient an empty report and then giving up.
            let attachments = resolve_attachments(&store, &payload.workspace, &json).await?;
            let meta = EmailMeta {
                workspace: payload.workspace.clone(),
                action,
            };

            for to in &payload.recipients {
                // Already delivered to THIS address on an earlier attempt? Then this retry is about
                // one of the others (or the ack was lost) — do not put a second copy in their inbox.
                match delivery_check(&store, &payload.workspace, EMAIL_TARGET, &dedup_key, to).await
                {
                    Ok(true) => continue,
                    Ok(false) => {}
                    Err(e) => {
                        return Err(DeliveryError::transient(format!(
                            "email target: delivered check: {e}"
                        )))
                    }
                }

                let message = EmailMessage {
                    to: to.clone(),
                    subject: content.subject.clone(),
                    text: content.text.clone(),
                    html: content.html.clone(),
                    attachments: attachments.clone(),
                    // The dedup key doubles as the cross-retry Message-ID (see the struct field).
                    message_id: Some(format!("{}@lazybones", sanitize_message_id(&dedup_key))),
                };
                provider.send(&message, &meta).await?;

                // Marked AFTER the provider reports success: a crash in between duplicates on retry,
                // which is the stated at-least-once window (delivered.rs), not a bug to hide. Marking
                // per recipient is what makes a partial fan-out failure safe to retry.
                delivery_mark(
                    &store,
                    &payload.workspace,
                    EMAIL_TARGET,
                    &dedup_key,
                    to,
                    effect_ts,
                )
                .await
                .map_err(|e| {
                    DeliveryError::transient(format!("email target: delivered mark: {e}"))
                })?;
            }
            Ok(())
        }
    }
}

/// Keep a `Message-ID` local part legal: the dedup key is an effect id like `invite:hash1`, and a raw
/// `:` or space in a `Message-ID` makes the header unparseable for the recipient that is supposed to be
/// deduping on it.
fn sanitize_message_id(dedup_key: &str) -> String {
    dedup_key
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::email_provider_dev::RecordingEmailProvider;
    use super::*;
    use lb_outbox::Effect;

    fn effect(action: &str, payload: serde_json::Value) -> Effect {
        Effect::new(
            "invite:hash1",
            EMAIL_TARGET,
            action,
            payload.to_string(),
            "invite:hash1",
            0,
        )
    }

    async fn rig() -> (Store, std::sync::Arc<RecordingEmailProvider>, EmailTarget) {
        let store = Store::memory().await.unwrap();
        let provider = std::sync::Arc::new(RecordingEmailProvider::default());
        let target = EmailTarget::new(Box::new(provider.clone()), store.clone());
        (store, provider, target)
    }

    #[tokio::test]
    async fn email_target_delivers_invite_with_both_body_halves() {
        let (_store, provider, target) = rig().await;
        let effect = effect(
            "send_invite",
            serde_json::json!({
                "email": "sam@example.com",
                "workspace": "nube",
                "token": "lbi_abc123",
            }),
        );

        target.deliver(&effect).await.unwrap();

        let sends = provider.sends();
        assert_eq!(sends.len(), 1);
        assert_eq!(sends[0].to, "sam@example.com");
        assert!(sends[0].body.contains("lbi_abc123"), "{:?}", sends[0].body);
        let html = sends[0].html.as_deref().expect("an HTML alternative");
        assert!(html.contains("lbi_abc123"), "{html}");
        // A stable, header-legal Message-ID derived from the idempotency key.
        assert_eq!(
            sends[0].message_id.as_deref(),
            Some("invite-hash1@lazybones")
        );
    }

    #[tokio::test]
    async fn a_payload_without_a_workspace_fails_permanently_rather_than_defaulting() {
        let (_store, provider, target) = rig().await;
        let effect = effect(
            "send_invite",
            serde_json::json!({ "email": "sam@example.com", "token": "lbi_abc123" }),
        );

        let err = target.deliver(&effect).await.expect_err("must refuse");
        assert!(err.permanent, "{err}");
        assert!(err.reason.contains("workspace"), "{err}");
        assert!(
            provider.sends().is_empty(),
            "nothing may be sent for an effect with no workspace"
        );
    }

    #[tokio::test]
    async fn a_retry_of_an_already_delivered_effect_does_not_send_twice() {
        let (_store, provider, target) = rig().await;
        let effect = effect(
            "send_invite",
            serde_json::json!({
                "email": "sam@example.com",
                "workspace": "nube",
                "token": "lbi_abc123",
            }),
        );

        target.deliver(&effect).await.unwrap();
        target.deliver(&effect).await.unwrap();
        assert_eq!(
            provider.sends().len(),
            1,
            "the delivered marker must suppress the second send"
        );
    }

    #[tokio::test]
    async fn a_report_effect_fans_out_to_every_recipient_with_the_pdf_attached() {
        let (store, provider, target) = rig().await;
        lb_assets::put_asset(
            &store,
            "nube",
            &lb_assets::Asset::new(
                "report-energy-week",
                "user:test",
                "application/pdf",
                b"%PDF-1.7 report bytes".to_vec(),
                1,
            ),
        )
        .await
        .unwrap();

        let effect = effect(
            "report",
            serde_json::json!({
                "workspace": "nube",
                "recipients": ["ap@nube-io.com", "ops@nube-io.com"],
                "subject": "energy — 2026-08-10 → 2026-08-17",
                "body": "The weekly energy report is attached.",
                "assetId": "report-energy-week",
            }),
        );

        target.deliver(&effect).await.unwrap();

        let sends = provider.sends();
        assert_eq!(sends.len(), 2, "one message per recipient");
        assert_eq!(sends[0].to, "ap@nube-io.com");
        assert_eq!(sends[1].to, "ops@nube-io.com");
        for send in &sends {
            assert_eq!(send.subject, "energy — 2026-08-10 → 2026-08-17");
            assert_eq!(send.body, "The weekly energy report is attached.");
            assert_eq!(
                send.attachments,
                vec![(
                    "report-energy-week.pdf".to_string(),
                    "application/pdf".to_string(),
                    21
                )],
                "the report itself must travel with the mail"
            );
        }
    }

    #[tokio::test]
    async fn a_retry_after_a_partial_fan_out_only_sends_to_whoever_missed_out() {
        let (_store, provider, target) = rig().await;
        let effect = effect(
            "report",
            serde_json::json!({
                "workspace": "nube",
                "recipients": ["first@nube-io.com", "second@nube-io.com"],
                "subject": "weekly",
            }),
        );

        // The FIRST send fails, so neither recipient has a ledger row yet.
        provider.fail_next(DeliveryError::transient("relay hiccup"));
        target
            .deliver(&effect)
            .await
            .expect_err("a failed recipient must fail the effect so the relay retries");
        assert!(
            provider.sends().is_empty(),
            "the failure was the first send, so nothing was recorded"
        );

        target.deliver(&effect).await.unwrap();
        let addressed: Vec<_> = provider.sends().into_iter().map(|s| s.to).collect();
        assert_eq!(addressed, vec!["first@nube-io.com", "second@nube-io.com"]);

        // A third pass is a no-op — both ledger rows are now written.
        target.deliver(&effect).await.unwrap();
        assert_eq!(provider.sends().len(), 2);
    }

    #[tokio::test]
    async fn an_effect_naming_a_missing_asset_mails_nobody() {
        let (_store, provider, target) = rig().await;
        let effect = effect(
            "report",
            serde_json::json!({
                "workspace": "nube",
                "recipients": ["ap@nube-io.com"],
                "assetId": "never-rendered",
            }),
        );

        let err = target.deliver(&effect).await.expect_err("must refuse");
        assert!(err.permanent, "{err}");
        assert!(
            provider.sends().is_empty(),
            "a report email with no report is worse than a visible dead letter"
        );
    }
}
