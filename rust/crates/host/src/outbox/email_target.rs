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
            let subject = match effect.action.as_str() {
                "send_invite" => "You're invited",
                _ => "Notification",
            };
            let body = format!(
                "You have been invited to workspace '{workspace}'.\n\nClick the link to accept:\n  /accept?token={token}\n"
            );
            provider.send(to, subject, &body, &meta).await
        }
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
