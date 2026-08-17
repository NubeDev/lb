//! [`PostmarkEmailProvider`] — the provider-API `impl EmailProvider`, beside the SMTP one.
//!
//! **Why a second impl at all.** Two reasons, both from the scope. First, it proves the trait admits both
//! shapes (a socket protocol and an HTTP API) rather than being an SMTP-shaped hole. Second, and more
//! practically: most products end up on a provider API, because **deliverability is not a code problem**.
//! Direct-to-MX from a node is filed as spam regardless of how correct the SMTP client is — no SPF/DKIM
//! alignment, no IP reputation, cloud IP blocks. A provider signs with the domain's key, keeps the
//! reputation, and reports bounces.
//!
//! **Why Postmark first** (the scope's open question, decided here): it is the simplest transactional API
//! in the field — one `POST /email` with a server-token header, no request signing — and it has the best
//! default deliverability posture for transactional mail. SES was the alternative and is the better fit for
//! an AWS-resident host, but it needs SigV4 request signing, which means either a large AWS SDK dependency
//! or a hand-rolled signer in a core crate; **and** SES exposes an SMTP submission interface, so an
//! AWS-resident host is already served by `kind: "smtp"` today with no new code. Mailgun/SendGrid are
//! near-identical shapes to this file if a host wants one — one file per provider.
//!
//! Bounce/complaint webhooks and a suppression list are named non-goals: they arrive over `POST /hooks`
//! (`ingest/webhooks-scope.md`) and want a suppression table, which is its own ask.

use std::time::Duration;

use async_trait::async_trait;
use base64ct::{Base64, Encoding};
use lb_store::Store;
use serde::Deserialize;

use super::delivery_error::DeliveryError;
use super::email_target::{EmailMessage, EmailMeta, EmailProvider};

/// Postmark's send endpoint. Overridable only so the tests can point at a real local HTTP server.
pub const POSTMARK_DEFAULT_ENDPOINT: &str = "https://api.postmarkapp.com/email";

/// The Postmark transport config — **names only** for the token, resolved per send like the SMTP one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostmarkConfig {
    /// The API endpoint (defaults to [`POSTMARK_DEFAULT_ENDPOINT`]).
    pub endpoint: String,
    /// The secrets PATH holding the Postmark **server token**.
    pub token_path: String,
    /// The env-var NAME holding the server token, as the node-level fallback.
    pub token_env: String,
    pub from_name: String,
    pub from_addr: String,
    pub reply_to: Option<String>,
    /// Postmark's message stream (`outbound` by default; a host separating transactional from broadcast
    /// traffic sets its own).
    pub message_stream: String,
    pub timeout: Duration,
}

impl Default for PostmarkConfig {
    fn default() -> Self {
        Self {
            endpoint: POSTMARK_DEFAULT_ENDPOINT.to_string(),
            token_path: String::new(),
            token_env: String::new(),
            from_name: String::new(),
            from_addr: String::new(),
            reply_to: None,
            message_stream: "outbound".to_string(),
            timeout: Duration::from_secs(super::provider_smtp::DEFAULT_SEND_TIMEOUT_SECS),
        }
    }
}

impl PostmarkConfig {
    /// Reject a config that cannot possibly send, at boot (see the SMTP twin for why this is loud).
    pub fn validate(&self) -> Result<(), String> {
        if self.from_addr.trim().is_empty() {
            return Err("email transport: postmark `from` address is empty".into());
        }
        if self.token_path.trim().is_empty() && self.token_env.trim().is_empty() {
            return Err(
                "email transport: postmark needs a server-token secret path or env name".into(),
            );
        }
        Ok(())
    }
}

/// Postmark's error body. Its `ErrorCode` is the machine-readable verdict we classify on.
#[derive(Debug, Deserialize)]
struct PostmarkError {
    #[serde(rename = "ErrorCode", default)]
    error_code: i64,
    #[serde(rename = "Message", default)]
    message: String,
}

/// The Postmark provider.
pub struct PostmarkEmailProvider {
    config: PostmarkConfig,
    store: Store,
    http: reqwest::Client,
}

impl PostmarkEmailProvider {
    pub fn new(config: PostmarkConfig, store: Store) -> Self {
        Self {
            config,
            store,
            http: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl EmailProvider for PostmarkEmailProvider {
    async fn send(&self, message: &EmailMessage, meta: &EmailMeta) -> Result<(), DeliveryError> {
        if meta.workspace.trim().is_empty() {
            return Err(DeliveryError::permanent(
                "email transport: no workspace on the effect — refusing to resolve a credential"
                    .to_string(),
            ));
        }
        let token = super::provider_smtp::resolve_secret(
            &self.store,
            &meta.workspace,
            &self.config.token_path,
            &self.config.token_env,
        )
        .await
        .ok_or_else(|| {
            DeliveryError::permanent(format!(
                "email transport: no postmark token at secret path '{}' (nor env '{}') for workspace {}",
                self.config.token_path, self.config.token_env, meta.workspace
            ))
        })?;

        let from = if self.config.from_name.trim().is_empty() {
            self.config.from_addr.clone()
        } else {
            format!("{} <{}>", self.config.from_name, self.config.from_addr)
        };
        let mut body = serde_json::json!({
            "From": from,
            "To": message.to,
            "Subject": message.subject,
            "TextBody": message.text,
            "MessageStream": self.config.message_stream,
        });
        if let Some(html) = message.html.as_deref().filter(|h| !h.trim().is_empty()) {
            body["HtmlBody"] = serde_json::Value::String(html.to_string());
        }
        if !message.attachments.is_empty() {
            // Postmark takes attachments inline, base64, in the same JSON body. Without this arm a
            // report email would arrive with its subject and its body and no report — the
            // silent-success failure mode this whole path exists to avoid.
            body["Attachments"] = serde_json::Value::Array(
                message
                    .attachments
                    .iter()
                    .map(|a| {
                        serde_json::json!({
                            "Name": a.filename,
                            "ContentType": a.mime,
                            "Content": Base64::encode_string(&a.bytes),
                        })
                    })
                    .collect(),
            );
        }
        if let Some(reply_to) = self.config.reply_to.as_deref() {
            body["ReplyTo"] = serde_json::Value::String(reply_to.to_string());
        }
        if let Some(id) = message.message_id.as_deref() {
            // The cross-retry dedup handle, as a custom header — Postmark controls `Message-ID` itself,
            // so this is the value a receiving side (or an operator reading a bounce) can correlate on.
            body["Headers"] = serde_json::json!([
                { "Name": "X-LB-Idempotency-Key", "Value": id }
            ]);
        }

        let response = self
            .http
            .post(&self.config.endpoint)
            .header("X-Postmark-Server-Token", &token)
            .header("Accept", "application/json")
            .timeout(self.config.timeout)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                // Unreachable API ⇒ retry. The error is reqwest's, which never carries our header value.
                DeliveryError::transient(format!("email transport: postmark unreachable: {e}"))
            })?;

        let status = response.status();
        if status.is_success() {
            tracing::info!(to = %message.to, ws = %meta.workspace, "email sent (postmark)");
            return Ok(());
        }

        let raw = response.text().await.unwrap_or_default();
        let parsed: Option<PostmarkError> = serde_json::from_str(&raw).ok();
        let (code, detail) = parsed
            .map(|p| (p.error_code, p.message))
            .unwrap_or((0, status.as_str().to_string()));
        // Classification, same contract as SMTP: a bad recipient / inactive address / unsigned sender
        // will not fix itself, while a 429 or a 5xx will. Postmark's own codes are the truth here —
        // 300 (invalid email), 406 (inactive recipient), 401 (bad token) are all operator work.
        let permanent = matches!(code, 300 | 401 | 406 | 409 | 500)
            || (status.is_client_error() && status.as_u16() != 429);
        let reason = format!(
            "postmark {} (code {code}): {}",
            status.as_u16(),
            // Postmark echoes the submitted address but never the token; the token is only ever a
            // header. Truncate anyway so an unexpectedly chatty body cannot bloat the outbox row.
            detail.chars().take(300).collect::<String>()
        );
        if permanent {
            Err(DeliveryError::permanent(reason))
        } else {
            Err(DeliveryError::transient(reason))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_refuses_a_transport_that_cannot_send() {
        let ok = PostmarkConfig {
            from_addr: "reports@nube.com".into(),
            token_path: "mail/postmark-token".into(),
            ..Default::default()
        };
        ok.validate().unwrap();
        assert!(PostmarkConfig {
            token_path: String::new(),
            ..ok.clone()
        }
        .validate()
        .is_err());
        assert!(PostmarkConfig {
            from_addr: String::new(),
            ..ok
        }
        .validate()
        .is_err());
    }
}
