//! The **email outbox target** — an `impl Target` that delivers invite emails (invites scope).
//! The email provider is the one sanctioned external: a true external you cannot run locally (an
//! SMTP relay / email API). It lives behind one trait (`EmailProvider`) in this one named file;
//! the test impl records sends (testing-scope §0 — the allow-list for fakes of true externals).
//!
//! No SMTP in core (invites scope: "one trait, one named file — the sanctioned external"). The
//! real provider is config — a product host wires its own `EmailProvider` impl (SMTP, Mailgun,
//! SES, …) and passes it to `spawn_relay_reactors`. The `RecordingEmailProvider` is the test impl.

use async_trait::async_trait;
use lb_outbox::Effect;
use serde::Deserialize;
use std::sync::Mutex;

use crate::outbox::Target;

/// The email provider — the one sanctioned external (invites scope). A product host wires its own
/// impl (SMTP/API); the test impl records sends. The trait is in this one named file so the fake
/// is clearly the allow-listed exception (testing-scope §0).
#[async_trait]
pub trait EmailProvider: Send + Sync {
    /// Send an email. `to` is the recipient email; `subject` + `body` are the final strings (no
    /// templating in core — invites scope non-goal). `meta` carries the workspace + token (opaque
    /// to the provider — it may log, never store the token).
    async fn send(
        &self,
        to: &str,
        subject: &str,
        body: &str,
        meta: &EmailMeta,
    ) -> Result<(), String>;
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
    async fn send(
        &self,
        to: &str,
        subject: &str,
        body: &str,
        meta: &EmailMeta,
    ) -> Result<(), String> {
        (**self).send(to, subject, body, meta).await
    }
}

/// The email `Target` adapter — reads the effect payload, calls the provider. Matches on
/// `effect.target == "email"` (the `EMAIL_TARGET` const).
pub struct EmailTarget {
    provider: Box<dyn EmailProvider>,
}

impl EmailTarget {
    pub fn new(provider: Box<dyn EmailProvider>) -> Self {
        Self { provider }
    }
}

impl Target for EmailTarget {
    fn deliver(
        &self,
        effect: &Effect,
    ) -> impl std::future::Future<Output = Result<(), String>> + Send {
        let payload = effect.payload.clone();
        let provider = &self.provider;
        async move {
            let payload: serde_json::Value = serde_json::from_str(&payload)
                .map_err(|e| format!("email target: bad payload json: {e}"))?;
            let to = payload
                .get("email")
                .and_then(|v| v.as_str())
                .ok_or("email target: missing email")?;
            let token = payload.get("token").and_then(|v| v.as_str()).unwrap_or("");
            let workspace = payload
                .get("workspace")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let meta = EmailMeta {
                workspace: workspace.to_string(),
                action: effect.action.clone(),
            };
            // Render subject/body through the prefs catalog engine in the effect's locale
            // (release scope, i18n gap b — the old "no templating in core" non-goal is
            // overturned by the multi-lang requirement; catalogs hold the words, the effect
            // holds the locale). An absent/unknown locale falls back to `en` in the resolver.
            let locale = payload.get("locale").and_then(|v| v.as_str()).unwrap_or("");
            let resolved = lb_prefs::resolve(&[lb_prefs::Prefs {
                language: Some(locale.to_string()),
                ..Default::default()
            }]);
            let args = serde_json::json!({ "workspace": workspace, "token": token });
            let empty = std::collections::BTreeMap::new();
            let (subject, body) = match effect.action.as_str() {
                "send_invite" => (
                    lb_prefs::render_message("invite.email.subject", &args, &empty, &resolved).text,
                    lb_prefs::render_message("invite.email.body", &args, &empty, &resolved).text,
                ),
                _ => ("Notification".to_string(), String::new()),
            };
            provider.send(to, &subject, &body, &meta).await
        }
    }
}

/// The **default boot provider** when no real one is configured (release scope, gap 1): logs the
/// send and acks it, so a node without email config boots and drains its outbox instead of
/// crashing or dead-lettering every invite. A product host replaces it via the boot provider seam.
pub struct LoggingEmailProvider;

#[async_trait]
impl EmailProvider for LoggingEmailProvider {
    async fn send(
        &self,
        to: &str,
        subject: &str,
        _body: &str,
        meta: &EmailMeta,
    ) -> Result<(), String> {
        tracing::info!(to = %to, subject = %subject, ws = %meta.workspace, "email (no provider configured — logged only)");
        Ok(())
    }
}

/// The recording test impl — records every send for assertion. The one sanctioned fake (a true
/// external behind a trait, testing-scope §0).
pub struct RecordingEmailProvider {
    sends: Mutex<Vec<RecordedEmail>>,
}

/// A recorded email send (for test assertion).
#[derive(Debug, Clone)]
pub struct RecordedEmail {
    pub to: String,
    pub subject: String,
    pub body: String,
    pub workspace: String,
}

impl Default for RecordingEmailProvider {
    fn default() -> Self {
        Self {
            sends: Mutex::new(Vec::new()),
        }
    }
}

impl RecordingEmailProvider {
    /// All recorded sends (in order).
    pub fn sends(&self) -> Vec<RecordedEmail> {
        self.sends.lock().unwrap().clone()
    }
}

#[async_trait]
impl EmailProvider for RecordingEmailProvider {
    async fn send(
        &self,
        to: &str,
        subject: &str,
        body: &str,
        meta: &EmailMeta,
    ) -> Result<(), String> {
        self.sends.lock().unwrap().push(RecordedEmail {
            to: to.to_string(),
            subject: subject.to_string(),
            body: body.to_string(),
            workspace: meta.workspace.clone(),
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lb_outbox::Effect;

    #[tokio::test]
    async fn email_target_delivers_invite() {
        let provider = RecordingEmailProvider::default();
        let target = EmailTarget::new(Box::new(provider));
        let payload = serde_json::json!({
            "email": "sam@example.com",
            "workspace": "acme",
            "token": "lbi_abc123",
        });
        let effect = Effect::new(
            "invite:hash1",
            "email",
            "send_invite",
            &payload.to_string(),
            "invite:hash1",
            0,
        );
        target.deliver(&effect).await.unwrap();
    }
}
