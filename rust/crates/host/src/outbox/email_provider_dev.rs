//! The two **non-transport** `EmailProvider` impls: the logging no-op and the recording test double.
//!
//! They live beside the real providers rather than inside `email_target.rs` because they are providers,
//! not target logic — the target file owns what happens *between* the effect row and the wire, and
//! these own neither.

use async_trait::async_trait;
use std::sync::Mutex;

use super::delivery_error::DeliveryError;
use super::email_target::{EmailMessage, EmailMeta, EmailProvider};

/// The **default boot provider** when no real one is configured: logs the send and acks it, so a node
/// without email config boots and drains its outbox instead of crashing or dead-lettering every effect.
///
/// **This was issue #118**: it was the only non-test impl, so every email the platform "sent" was
/// logged and dropped — an admin invited a colleague, the outbox drained clean, and the colleague was
/// never told. It survives as an *explicit* `kind: "logging"` choice for dev, and boot warns loudly when
/// it is in use by default (`node/src/mail.rs`), because a silent success is worse than a failure: it
/// strands nothing and delivers nothing.
pub struct LoggingEmailProvider;

#[async_trait]
impl EmailProvider for LoggingEmailProvider {
    async fn send(&self, message: &EmailMessage, meta: &EmailMeta) -> Result<(), DeliveryError> {
        tracing::warn!(
            to = %message.to, subject = %message.subject, ws = %meta.workspace,
            attachments = message.attachments.len(),
            "email DROPPED (no transport configured — logged only; set the boot email transport)"
        );
        Ok(())
    }
}

/// The recording test impl — records every send for assertion, and can be scripted to fail. The one
/// sanctioned fake (a true external behind a trait, testing-scope §0).
///
/// It proves the *target's* behaviour (rendering, dedup, fan-out, workspace refusal), never the
/// transport's: asserting our own recorder says nothing about TLS/auth/MIME, which is what `lb-mail`'s
/// tests against a real SMTP server are for.
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
    /// `(filename, mime, byte length)` per attachment — the bytes themselves are not copied into every
    /// assertion, but their length is what proves the artefact actually travelled.
    pub attachments: Vec<(String, String, usize)>,
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
            attachments: message
                .attachments
                .iter()
                .map(|a| (a.filename.clone(), a.mime.clone(), a.bytes.len()))
                .collect(),
        });
        Ok(())
    }
}
