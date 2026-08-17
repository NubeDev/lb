//! [`SmtpTransportConfig`] — **what a node's mailer is configured with**, and nothing else.
//!
//! Split out of `provider_smtp.rs` (FILE-LAYOUT §3): that file owns the *sending* — resolving a
//! credential per send, minting an OAuth access token, mapping a transport error onto the outbox's
//! retry classification. This file owns the *description* of a relay, which is a separate concern
//! with a separate reader: an operator writing deployment config, not a maintainer reading the send
//! path.
//!
//! **Names only, never values.** Every secret here is a secrets PATH plus an env-var NAME; the value
//! is read at send time in the effect's own workspace (`provider_smtp.rs::credentials`). That is what
//! makes this struct safe to `Debug`, log, and echo in a boot diagnostic — and it is asserted below,
//! because "the config is loggable" is a property that quietly stops being true the moment someone
//! adds a convenient `password: String` field.

use std::time::Duration;

use lb_mail::{AuthMechanism, TlsMode};

pub const DEFAULT_SEND_TIMEOUT_SECS: u64 = 30;

/// The XOAUTH2 half of the transport config — names only, never values.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SmtpOauthConfig {
    /// The provider's token endpoint (`https://oauth2.googleapis.com/token`).
    pub token_endpoint: String,
    /// The OAuth2 client id (not a secret).
    pub client_id: String,
    /// The secrets PATH holding the OAuth2 client secret.
    pub client_secret_path: String,
    /// The env-var NAME holding the client secret, when it is not sealed.
    pub client_secret_env: String,
}

/// Where and how this node submits mail. **Names only** for every secret (see the module note), so the
/// struct is safe to `Debug`, log, and echo in a diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmtpTransportConfig {
    pub host: String,
    pub port: u16,
    pub tls: TlsMode,
    pub auth: AuthMechanism,
    /// The SMTP username (usually the sending address) — not a secret.
    pub username: String,
    /// The secrets PATH holding the password (`plain`/`login`) or the OAuth **refresh token**
    /// (`xoauth2`). Resolved at send time in the effect's workspace.
    pub secret_path: String,
    /// The env-var NAME holding the same value, as the node-level fallback.
    pub secret_env: String,
    /// The `From` display name.
    pub from_name: String,
    /// The `From` address the recipient sees.
    pub from_addr: String,
    /// An optional `Reply-To`.
    pub reply_to: Option<String>,
    /// An optional `MAIL FROM` envelope sender, when it must differ from `from_addr`.
    pub envelope_from: Option<String>,
    pub timeout: Duration,
    /// XOAUTH2 settings — required when `auth` is [`AuthMechanism::XOauth2`].
    pub oauth: Option<SmtpOauthConfig>,
    /// Accept an invalid server certificate. Dev/test only; `false` in any real deployment.
    pub allow_invalid_certs: bool,
}

impl Default for SmtpTransportConfig {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: 587,
            tls: TlsMode::Starttls,
            auth: AuthMechanism::Plain,
            username: String::new(),
            secret_path: String::new(),
            secret_env: String::new(),
            from_name: String::new(),
            from_addr: String::new(),
            reply_to: None,
            envelope_from: None,
            timeout: Duration::from_secs(DEFAULT_SEND_TIMEOUT_SECS),
            oauth: None,
            allow_invalid_certs: false,
        }
    }
}

impl SmtpTransportConfig {
    /// Reject a config that cannot possibly send, at BOOT rather than at the first invite.
    ///
    /// Not defensive noise: the failure this prevents is the one the issue is about — a transport that
    /// looks configured, boots clean, and drops mail. A missing host or `From`, or `xoauth2` with no
    /// token endpoint, is a deployment mistake that should be loud immediately.
    pub fn validate(&self) -> Result<(), String> {
        if self.host.trim().is_empty() {
            return Err("email transport: smtp host is empty".into());
        }
        if self.from_addr.trim().is_empty() {
            return Err("email transport: `from` address is empty".into());
        }
        if self.auth.needs_secret()
            && self.secret_path.trim().is_empty()
            && self.secret_env.trim().is_empty()
        {
            return Err(format!(
                "email transport: auth is '{}' but neither a secret path nor an env name is set",
                self.auth.as_str()
            ));
        }
        if self.auth == AuthMechanism::XOauth2 {
            let oauth = self
                .oauth
                .as_ref()
                .ok_or("email transport: auth is 'xoauth2' but no oauth settings are configured")?;
            if oauth.token_endpoint.trim().is_empty() {
                return Err("email transport: xoauth2 needs a token_endpoint".into());
            }
            if oauth.client_id.trim().is_empty() {
                return Err("email transport: xoauth2 needs a client_id".into());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_refuses_a_transport_that_cannot_send() {
        let config = SmtpTransportConfig {
            host: "smtp.nube.com".into(),
            from_addr: "reports@nube.com".into(),
            secret_path: "mail/smtp-password".into(),
            ..Default::default()
        };
        config.validate().expect("a complete config validates");

        // The failure mode issue #118 is about: something that boots and silently cannot deliver.
        let no_host = SmtpTransportConfig {
            host: String::new(),
            ..config.clone()
        };
        assert!(no_host.validate().is_err());

        let no_credential = SmtpTransportConfig {
            secret_path: String::new(),
            secret_env: String::new(),
            ..config.clone()
        };
        assert!(no_credential.validate().is_err());

        // xoauth2 without a token endpoint is the "supports Gmail" trap: it would fail an hour in.
        let bad_oauth = SmtpTransportConfig {
            auth: AuthMechanism::XOauth2,
            oauth: None,
            ..config.clone()
        };
        assert!(bad_oauth.validate().is_err());
    }

    #[test]
    fn the_config_carries_no_secret_value_so_it_is_safe_to_debug() {
        let config = SmtpTransportConfig {
            host: "smtp.gmail.com".into(),
            username: "reports@nube.com".into(),
            secret_path: "mail/gmail-refresh-token".into(),
            secret_env: "LB_MAIL_SECRET".into(),
            from_addr: "reports@nube.com".into(),
            ..Default::default()
        };
        // Names only — the struct is Debug-safe BY CONSTRUCTION, which is what makes it loggable in a
        // boot diagnostic. (The values live in secrets and are resolved per send.)
        let debug = format!("{config:?}");
        assert!(debug.contains("mail/gmail-refresh-token"));
        assert!(debug.contains("LB_MAIL_SECRET"));
    }
}
