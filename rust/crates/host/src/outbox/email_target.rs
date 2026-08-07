//! The **email outbox target** — an `impl Target` that delivers invite (and future) emails.
//!
//! The provider is the one sanctioned external: a true external you cannot run locally (an SMTP relay /
//! an email API). It lives behind one trait ([`EmailProvider`]) in this one named file; the test impl
//! records sends (testing-scope §0 — the allow-list for fakes of true externals). Real impls live beside
//! this file: [`SmtpEmailProvider`](super::SmtpEmailProvider) and
//! [`PostmarkEmailProvider`](super::PostmarkEmailProvider), both selected by boot config.
//!
//! What this file owns is everything *between* the effect row and the wire:
//! - render subject/body/html from the `lb_prefs` catalog in the **effect's** locale,
//! - refuse to guess a workspace (rule 6) — an effect without one fails rather than defaulting,
//! - dedup a retry against the delivered ledger so an accepted message is not sent twice,
//! - hand the provider a stable `Message-ID` so the *receiving* side can collapse a duplicate too.
//!
//! It owns no transport: no socket, no credential, no provider name (rule 10 — the effect's `target`
//! string is opaque routing data and this file is reached only through it).

use async_trait::async_trait;
use lb_outbox::Effect;
use lb_store::Store;
use serde::Deserialize;
use std::sync::Mutex;

use super::delivered::{delivery_check, delivery_mark};
use super::delivery_error::DeliveryError;
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
    /// The recipient address.
    pub to: String,
    pub subject: String,
    /// The plain-text body. Always populated.
    pub text: String,
    /// The optional HTML body (from the catalog's `*_html` key).
    pub html: Option<String>,
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

/// The email `Target` adapter — reads the effect payload, renders the catalog, calls the provider.
/// Matches on `effect.target == "email"` (the [`EMAIL_TARGET`] const).
pub struct EmailTarget {
    provider: Box<dyn EmailProvider>,
    /// For the delivered ledger (retry dedup). The target holds a store handle for the same reason
    /// `PushTarget` does — the dedup marker is durable state, and losing it means re-sending.
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
        let payload = effect.payload.clone();
        let action = effect.action.clone();
        let effect_id = effect.id.clone();
        let idempotency_key = effect.idempotency_key.clone();
        let effect_ts = effect.ts;
        let provider = &self.provider;
        let store = self.store.clone();
        async move {
            let payload: serde_json::Value = serde_json::from_str(&payload).map_err(|e| {
                // A payload that is not JSON will never become JSON — no retry.
                DeliveryError::permanent(format!("email target: bad payload json: {e}"))
            })?;
            let to = payload
                .get("email")
                .and_then(|v| v.as_str())
                .filter(|e| !e.trim().is_empty())
                .ok_or_else(|| {
                    DeliveryError::permanent("email target: payload missing email".to_string())
                })?;
            let token = payload.get("token").and_then(|v| v.as_str()).unwrap_or("");
            // The workspace comes from the PAYLOAD and is never defaulted (rule 6, the hard wall). The
            // cautionary tale is `push_target`'s hardcoded workspace: an effect delivered under a
            // guessed workspace resolves another tenant's config/secrets. Absent ⇒ fail the effect.
            let workspace = payload
                .get("workspace")
                .and_then(|v| v.as_str())
                .filter(|w| !w.trim().is_empty())
                .ok_or_else(|| {
                    DeliveryError::permanent(
                        "email target: payload missing workspace — refusing to guess (rule 6)"
                            .to_string(),
                    )
                })?
                .to_string();

            // Retry-dedup key: the outbox's own idempotency handle (falls back to the effect id).
            let dedup_key = if idempotency_key.is_empty() {
                effect_id
            } else {
                idempotency_key
            };

            // Already delivered on an earlier attempt? Then this retry is about something else (or the
            // ack was lost) — do not put a second copy on the wire.
            match delivery_check(&store, &workspace, EMAIL_TARGET, &dedup_key, to).await {
                Ok(true) => return Ok(()),
                Ok(false) => {}
                Err(e) => {
                    return Err(DeliveryError::transient(format!(
                        "email target: delivered check: {e}"
                    )))
                }
            }

            // Render subject/body through the prefs catalog engine in the effect's locale (release
            // scope, i18n gap b — the old "no templating in core" non-goal is overturned by the
            // multi-lang requirement; catalogs hold the words, the effect holds the locale). An
            // absent/unknown locale falls back to `en` in the resolver.
            let locale = payload.get("locale").and_then(|v| v.as_str()).unwrap_or("");
            let resolved = lb_prefs::resolve(&[lb_prefs::Prefs {
                language: Some(locale.to_string()),
                ..Default::default()
            }]);
            let args = serde_json::json!({ "workspace": workspace, "token": token });
            let empty = std::collections::BTreeMap::new();
            let render = |key: &str| lb_prefs::render_message(key, &args, &empty, &resolved).text;
            let (subject, text, html) = match action.as_str() {
                "send_invite" => (
                    render("invite.email.subject"),
                    render("invite.email.body"),
                    catalog_html(&render("invite.email.body_html"), "invite.email.body_html"),
                ),
                _ => ("Notification".to_string(), String::new(), None),
            };

            let message = EmailMessage {
                to: to.to_string(),
                subject,
                text,
                html,
                // The dedup key doubles as the cross-retry Message-ID (see the struct field).
                message_id: Some(format!("{}@lazybones", sanitize_message_id(&dedup_key))),
            };
            let meta = EmailMeta {
                workspace: workspace.clone(),
                action,
            };
            provider.send(&message, &meta).await?;

            // Marked AFTER the provider reports success: a crash in between duplicates on retry, which
            // is the stated at-least-once window (delivered.rs), not a bug to hide.
            delivery_mark(&store, &workspace, EMAIL_TARGET, &dedup_key, to, effect_ts)
                .await
                .map_err(|e| {
                    DeliveryError::transient(format!("email target: delivered mark: {e}"))
                })?;
            Ok(())
        }
    }
}

/// The catalog's HTML half, or `None` when it isn't there.
///
/// `lb_prefs::render` never returns blank — an absent key renders as **the key literal** (its documented
/// last fallback). That is right for a subject line and wrong for an optional HTML body, so this is where
/// "the catalog has no `*_html` key" is turned back into `None` and the message goes out text-only.
fn catalog_html(rendered: &str, key: &str) -> Option<String> {
    let trimmed = rendered.trim();
    if trimmed.is_empty() || trimmed == key {
        return None;
    }
    Some(rendered.to_string())
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

/// The **default boot provider** when no real one is configured (release scope, gap 1): logs the
/// send and acks it, so a node without email config boots and drains its outbox instead of
/// crashing or dead-lettering every invite.
///
/// **This was issue #118**: it was the only non-test impl, so every email the platform "sent" was
/// logged and dropped — an admin invited a colleague, the outbox drained clean, and the colleague was
/// never told. It survives as an *explicit* `kind: "logging"` choice for dev, and boot now warns loudly
/// when it is in use by default (`node/src/mail.rs`), because a silent success is worse than a failure:
/// it strands nothing and delivers nothing.
pub struct LoggingEmailProvider;

#[async_trait]
impl EmailProvider for LoggingEmailProvider {
    async fn send(&self, message: &EmailMessage, meta: &EmailMeta) -> Result<(), DeliveryError> {
        tracing::warn!(
            to = %message.to, subject = %message.subject, ws = %meta.workspace,
            "email DROPPED (no transport configured — logged only; set the boot email transport)"
        );
        Ok(())
    }
}

/// The recording test impl — records every send for assertion, and can be scripted to fail. The one
/// sanctioned fake (a true external behind a trait, testing-scope §0).
///
/// It proves the *target's* behaviour (rendering, dedup, workspace refusal), never the transport's:
/// asserting our own recorder says nothing about TLS/auth/MIME, which is what `lb-mail`'s tests against
/// a real SMTP server are for.
pub struct RecordingEmailProvider {
    sends: Mutex<Vec<RecordedEmail>>,
    fail_next: Mutex<Option<DeliveryError>>,
}

/// A recorded email send (for test assertion).
#[derive(Debug, Clone)]
pub struct RecordedEmail {
    pub to: String,
    pub subject: String,
    pub body: String,
    pub html: Option<String>,
    pub message_id: Option<String>,
    pub workspace: String,
}

impl Default for RecordingEmailProvider {
    fn default() -> Self {
        Self {
            sends: Mutex::new(Vec::new()),
            fail_next: Mutex::new(None),
        }
    }
}

impl RecordingEmailProvider {
    /// All recorded sends (in order).
    pub fn sends(&self) -> Vec<RecordedEmail> {
        self.sends.lock().unwrap().clone()
    }

    /// Script the NEXT send to fail with `error` (then succeed) — so the relay's retry/park paths are
    /// exercised through the REAL relay loop.
    pub fn fail_next(&self, error: DeliveryError) {
        *self.fail_next.lock().unwrap() = Some(error);
    }
}

#[async_trait]
impl EmailProvider for RecordingEmailProvider {
    async fn send(&self, message: &EmailMessage, meta: &EmailMeta) -> Result<(), DeliveryError> {
        if let Some(error) = self.fail_next.lock().unwrap().take() {
            return Err(error);
        }
        self.sends.lock().unwrap().push(RecordedEmail {
            to: message.to.clone(),
            subject: message.subject.clone(),
            body: message.text.clone(),
            html: message.html.clone(),
            message_id: message.message_id.clone(),
            workspace: meta.workspace.clone(),
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lb_outbox::Effect;

    fn invite_effect(payload: serde_json::Value) -> Effect {
        Effect::new(
            "invite:hash1",
            EMAIL_TARGET,
            "send_invite",
            payload.to_string(),
            "invite:hash1",
            0,
        )
    }

    #[tokio::test]
    async fn email_target_delivers_invite_with_both_body_halves() {
        let store = Store::memory().await.unwrap();
        let provider = std::sync::Arc::new(RecordingEmailProvider::default());
        let target = EmailTarget::new(Box::new(provider.clone()), store);
        let effect = invite_effect(serde_json::json!({
            "email": "sam@example.com",
            "workspace": "nube",
            "token": "lbi_abc123",
        }));

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
        let store = Store::memory().await.unwrap();
        let provider = std::sync::Arc::new(RecordingEmailProvider::default());
        let target = EmailTarget::new(Box::new(provider.clone()), store);
        let effect = invite_effect(serde_json::json!({
            "email": "sam@example.com",
            "token": "lbi_abc123",
        }));

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
        let store = Store::memory().await.unwrap();
        let provider = std::sync::Arc::new(RecordingEmailProvider::default());
        let target = EmailTarget::new(Box::new(provider.clone()), store);
        let effect = invite_effect(serde_json::json!({
            "email": "sam@example.com",
            "workspace": "nube",
            "token": "lbi_abc123",
        }));

        target.deliver(&effect).await.unwrap();
        target.deliver(&effect).await.unwrap();
        assert_eq!(
            provider.sends().len(),
            1,
            "the delivered marker must suppress the second send"
        );
    }
}
