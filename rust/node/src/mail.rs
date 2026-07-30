//! The **email transport selector** — turn boot config into an `EmailProvider`.
//!
//! This is the last mile of issue #118. The transport impls live in `lb-host`
//! (`outbox/provider_smtp.rs`, `outbox/provider_postmark.rs`); this file is the *binary-boundary*
//! decision of which one a node runs, so a host gets a working mailer **from configuration alone**
//! instead of writing Rust to implement a trait. The existing
//! [`OutboxProviders::email`](crate::config::OutboxProviders::email) seam stays as the escape hatch for a
//! host with its own transport, and it WINS over this config (an embedder that handed us a provider meant
//! it).
//!
//! Two design points worth stating:
//!
//! - **An unset transport is a loud warning, not a silent success.** `None` still boots with the logging
//!   provider — a node must not crash or dead-letter every invite because nobody configured a mailer —
//!   but it says so, at `warn`, naming the config it wants. Silence is what let #118 live: the outbox
//!   drained clean and nobody was ever told. An operator who genuinely wants log-only in dev asks for
//!   `kind: "logging"` explicitly and gets no warning.
//! - **A misconfigured transport fails at boot, not at the first invite.** `validate()` runs here, and a
//!   config that cannot possibly send (no host, no `From`, `xoauth2` without a token endpoint) is
//!   reported loudly and falls back to logging rather than pretending. The node still boots — refusing
//!   to boot a whole node over a mail typo is worse — but the log line is unambiguous.
//!
//! Env parsing lives here too, at the binary boundary (the `LB_MAIL_*` vars), per the doctrine that no
//! library code below the boot seam reads env.

use std::time::Duration;

use lb_host::{
    EmailProvider, LoggingEmailProvider, MailAuthMechanism as AuthMechanism, PostmarkConfig,
    PostmarkEmailProvider, SmtpEmailProvider, SmtpOauthConfig, SmtpTransportConfig, Store, TlsMode,
};

/// Which transport this node submits mail through — selected **by name** in boot config.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EmailTransport {
    /// Log the send and ack it. The pre-#118 default behaviour, kept as an EXPLICIT dev choice: chosen
    /// deliberately it is honest (and quiet); arrived at by omission it is the bug.
    Logging,
    /// A real SMTP relay (`kind: "smtp"`) — the portable floor, including Gmail/M365 via XOAUTH2.
    Smtp(SmtpTransportConfig),
    /// The Postmark transactional API (`kind: "postmark"`) — the deliverability path.
    Postmark(PostmarkConfig),
}

impl EmailTransport {
    /// The config-facing name, for logs.
    pub fn kind(&self) -> &'static str {
        match self {
            EmailTransport::Logging => "logging",
            EmailTransport::Smtp(_) => "smtp",
            EmailTransport::Postmark(_) => "postmark",
        }
    }

    /// Reject a transport that cannot possibly send (see the module note).
    pub fn validate(&self) -> Result<(), String> {
        match self {
            EmailTransport::Logging => Ok(()),
            EmailTransport::Smtp(c) => c.validate(),
            EmailTransport::Postmark(c) => c.validate(),
        }
    }
}

/// Build the provider the relay reactor delivers email through.
///
/// Precedence: an embedder-supplied provider (the escape hatch) → the configured transport → the logging
/// provider with a loud warning.
pub fn build_email_provider(
    embedder: Option<&std::sync::Arc<dyn EmailProvider>>,
    transport: Option<&EmailTransport>,
    store: &Store,
) -> Box<dyn EmailProvider> {
    if let Some(provider) = embedder {
        tracing::info!("email transport: using the embedder-supplied EmailProvider");
        return Box::new(provider.clone());
    }
    let Some(transport) = transport else {
        tracing::warn!(
            "email transport: NONE configured — every email will be logged and DROPPED. Set the boot \
             email transport (kind: \"smtp\" | \"postmark\", or \"logging\" to silence this) — see \
             docs/skills/email-transport/SKILL.md"
        );
        return Box::new(LoggingEmailProvider);
    };
    if let Err(problem) = transport.validate() {
        tracing::error!(
            kind = transport.kind(),
            %problem,
            "email transport: MISCONFIGURED — falling back to logging, so email is DROPPED until fixed"
        );
        return Box::new(LoggingEmailProvider);
    }
    match transport {
        EmailTransport::Logging => {
            tracing::info!("email transport: logging (explicit) — email is not delivered");
            Box::new(LoggingEmailProvider)
        }
        EmailTransport::Smtp(config) => {
            tracing::info!(
                host = %config.host, port = config.port, tls = config.tls.as_str(),
                auth = config.auth.as_str(), from = %config.from_addr,
                "email transport: smtp"
            );
            Box::new(SmtpEmailProvider::new(config.clone(), store.clone()))
        }
        EmailTransport::Postmark(config) => {
            tracing::info!(
                endpoint = %config.endpoint, from = %config.from_addr,
                stream = %config.message_stream,
                "email transport: postmark"
            );
            Box::new(PostmarkEmailProvider::new(config.clone(), store.clone()))
        }
    }
}

/// Read the transport from `LB_MAIL_*` env — the binary boundary's job, never a library's.
///
/// `LB_MAIL_KIND` selects; everything else fills the selected shape. Absent/empty ⇒ `None` (unset, which
/// warns at boot). An unknown kind is reported and treated as unset rather than guessed.
pub fn email_transport_from_env() -> Option<EmailTransport> {
    let kind = env_value("LB_MAIL_KIND")?.to_ascii_lowercase();
    match kind.as_str() {
        "logging" | "log" | "none" => Some(EmailTransport::Logging),
        "smtp" => Some(EmailTransport::Smtp(smtp_from_env())),
        "postmark" => Some(EmailTransport::Postmark(postmark_from_env())),
        other => {
            tracing::error!(
                kind = %other,
                "LB_MAIL_KIND is not a known transport (smtp | postmark | logging) — treating email as unconfigured"
            );
            None
        }
    }
}

fn smtp_from_env() -> SmtpTransportConfig {
    let (from_name, from_addr) = parse_from(env_value("LB_MAIL_FROM").unwrap_or_default());
    // A bad tls/auth string is reported and the safe default kept: STARTTLS + PLAIN is what a hosted
    // relay wants, and silently downgrading TLS on a typo is the one outcome we will not have.
    let tls = match TlsMode::parse(&env_value("LB_MAIL_TLS").unwrap_or_default()) {
        Ok(t) => t,
        Err(e) => {
            tracing::error!(problem = %e.message(), "LB_MAIL_TLS — keeping starttls");
            TlsMode::Starttls
        }
    };
    let auth = match AuthMechanism::parse(&env_value("LB_MAIL_AUTH").unwrap_or_default()) {
        Ok(a) => a,
        Err(e) => {
            tracing::error!(problem = %e.message(), "LB_MAIL_AUTH — keeping plain");
            AuthMechanism::Plain
        }
    };
    let oauth = (auth == AuthMechanism::XOauth2).then(|| SmtpOauthConfig {
        token_endpoint: env_value("LB_MAIL_OAUTH_TOKEN_ENDPOINT").unwrap_or_default(),
        client_id: env_value("LB_MAIL_OAUTH_CLIENT_ID").unwrap_or_default(),
        client_secret_path: env_value("LB_MAIL_OAUTH_CLIENT_SECRET_PATH").unwrap_or_default(),
        client_secret_env: env_value("LB_MAIL_OAUTH_CLIENT_SECRET_ENV").unwrap_or_default(),
    });
    SmtpTransportConfig {
        host: env_value("LB_MAIL_HOST").unwrap_or_default(),
        port: env_value("LB_MAIL_PORT")
            .and_then(|p| p.parse().ok())
            .unwrap_or(587),
        tls,
        auth,
        username: env_value("LB_MAIL_USER").unwrap_or_default(),
        secret_path: env_value("LB_MAIL_SECRET_PATH").unwrap_or_default(),
        secret_env: env_value("LB_MAIL_SECRET_ENV").unwrap_or_default(),
        from_name,
        from_addr,
        reply_to: env_value("LB_MAIL_REPLY_TO"),
        envelope_from: env_value("LB_MAIL_ENVELOPE_FROM"),
        timeout: env_value("LB_MAIL_TIMEOUT_SECS")
            .and_then(|t| t.parse().ok())
            .map(Duration::from_secs)
            .unwrap_or_else(|| Duration::from_secs(lb_host::DEFAULT_SEND_TIMEOUT_SECS)),
        oauth,
        allow_invalid_certs: env_value("LB_MAIL_ALLOW_INVALID_CERTS").is_some_and(|v| v == "1"),
    }
}

fn postmark_from_env() -> PostmarkConfig {
    let (from_name, from_addr) = parse_from(env_value("LB_MAIL_FROM").unwrap_or_default());
    let defaults = PostmarkConfig::default();
    PostmarkConfig {
        endpoint: env_value("LB_MAIL_ENDPOINT").unwrap_or(defaults.endpoint),
        token_path: env_value("LB_MAIL_SECRET_PATH").unwrap_or_default(),
        token_env: env_value("LB_MAIL_SECRET_ENV").unwrap_or_default(),
        from_name,
        from_addr,
        reply_to: env_value("LB_MAIL_REPLY_TO"),
        message_stream: env_value("LB_MAIL_STREAM").unwrap_or(defaults.message_stream),
        timeout: env_value("LB_MAIL_TIMEOUT_SECS")
            .and_then(|t| t.parse().ok())
            .map(Duration::from_secs)
            .unwrap_or(defaults.timeout),
    }
}

/// Split `Acme <reports@acme.com>` into `("Acme", "reports@acme.com")`; a bare address yields no name.
fn parse_from(value: String) -> (String, String) {
    let trimmed = value.trim();
    match (trimmed.find('<'), trimmed.rfind('>')) {
        (Some(open), Some(close)) if close > open => (
            trimmed[..open].trim().trim_matches('"').to_string(),
            trimmed[open + 1..close].trim().to_string(),
        ),
        _ => (String::new(), trimmed.to_string()),
    }
}

fn env_value(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_header_parses_a_display_name_or_a_bare_address() {
        assert_eq!(
            parse_from("Acme Reports <reports@acme.com>".into()),
            ("Acme Reports".to_string(), "reports@acme.com".to_string())
        );
        assert_eq!(
            parse_from("reports@acme.com".into()),
            (String::new(), "reports@acme.com".to_string())
        );
        assert_eq!(
            parse_from("\"Acme, Inc\" <reports@acme.com>".into()),
            ("Acme, Inc".to_string(), "reports@acme.com".to_string())
        );
    }

    #[tokio::test]
    async fn an_unset_transport_still_boots_on_the_logging_provider() {
        // The whole point of keeping the logging provider: a node with no mail config must boot and
        // drain its outbox, never crash and never strand effects. It just says so loudly now.
        let store = Store::memory().await.unwrap();
        let _provider = build_email_provider(None, None, &store);
    }

    #[tokio::test]
    async fn a_misconfigured_transport_falls_back_to_logging_instead_of_pretending() {
        let store = Store::memory().await.unwrap();
        // No host, no From: validate() rejects it and boot does not hand the relay a provider that
        // would fail every single send.
        let broken = EmailTransport::Smtp(SmtpTransportConfig::default());
        assert!(broken.validate().is_err());
        let _provider = build_email_provider(None, Some(&broken), &store);
    }
}
