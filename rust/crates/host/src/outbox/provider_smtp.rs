//! [`SmtpEmailProvider`] — the first **real** `impl EmailProvider`: an SMTP relay behind the trait.
//!
//! This is the fix for issue #118. The seam was fully built and booted — a durable outbox effect,
//! `RouterTarget` dispatch on the opaque `"email"` string, `EmailTarget`, catalog-rendered i18n — and the
//! only non-test provider logged the send and *acked* it. Nothing here changes the seam; it fills it.
//!
//! **SMTP, not only a provider API, on purpose.** On-prem and edge deployments (a real posture for this
//! platform) often have an internal relay and no egress to a SaaS API. SMTP is the portable floor;
//! [`PostmarkEmailProvider`](super::PostmarkEmailProvider) is the deliverability path beside it.
//!
//! ## Credentials are resolved per send, by path
//!
//! The config struct holds **names only** — a secrets path and an env-var name — and the value is read
//! at send time through the same precedence as every other host-resolved credential
//! (`agent/resolve_key.rs`): **sealed workspace secret → node env → unset**. The workspace comes from the
//! effect payload (`EmailMeta`), never from ambient state, so a ws-A effect can only ever resolve ws-A's
//! sealed secret (rule 6). Nothing is cached except the OAuth access token, which is keyed by a digest of
//! the refresh token so rotating the seal invalidates it.
//!
//! An `unset` credential where the mechanism needs one is a **permanent** failure, not a retry: booting a
//! mailer whose secret path is a typo is a config bug, and retrying it five times just delays the
//! dead-letter row that tells the operator.
//!
//! ## Deliverability is not a code problem
//!
//! Sending direct-to-MX from a node gets filed as spam no matter how correct this file is — no SPF/DKIM
//! alignment, no IP reputation. The recommendation to hosts is a relay or a provider API with the
//! domain's DNS set up; see `docs/skills/email-transport/SKILL.md`.

use std::time::Duration;

use async_trait::async_trait;
use lb_mail::send::auth::{access_token, MailCredentials, RefreshRequest, TokenCache};
use lb_mail::{send_smtp, AuthMechanism, MailError, MailMessage, SmtpEndpoint, TlsMode};
use lb_store::Store;

use super::delivery_error::DeliveryError;
use super::email_target::{EmailMessage, EmailMeta, EmailProvider};

/// The default per-send timeout. Bounded on purpose and never `None`: an SMTP session hanging inside the
/// relay tick stalls *every* outbox delivery behind it, push included.
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

/// The SMTP provider. One per node; holds the config (names only), an HTTP client for token refresh,
/// and the access-token cache.
pub struct SmtpEmailProvider {
    config: SmtpTransportConfig,
    store: Store,
    http: reqwest::Client,
    tokens: TokenCache,
}

impl SmtpEmailProvider {
    /// Build the provider. `store` is needed to resolve the sealed secret **in the effect's workspace**
    /// at send time (never at construction — a credential held from boot is a credential in a core dump).
    pub fn new(config: SmtpTransportConfig, store: Store) -> Self {
        Self {
            config,
            store,
            http: reqwest::Client::new(),
            tokens: TokenCache::new(),
        }
    }

    /// Resolve the credential for a send in `ws`: **sealed workspace secret → node env → unset**, the
    /// same precedence as every other host-resolved credential.
    async fn credentials(&self, ws: &str) -> Result<MailCredentials, DeliveryError> {
        if self.config.auth == AuthMechanism::None {
            return Ok(MailCredentials::None);
        }
        let secret = resolve_secret(
            &self.store,
            ws,
            &self.config.secret_path,
            &self.config.secret_env,
        )
        .await
        .ok_or_else(|| {
            // Permanent: a path/env that resolves to nothing is a config error. Note the message names
            // the PATH, never the value.
            DeliveryError::permanent(format!(
                "email transport: no credential at secret path '{}' (nor env '{}') for workspace {ws}",
                self.config.secret_path, self.config.secret_env
            ))
        })?;

        match self.config.auth {
            AuthMechanism::None => Ok(MailCredentials::None),
            AuthMechanism::Plain | AuthMechanism::Login => Ok(MailCredentials::Password {
                username: self.config.username.clone(),
                password: secret,
            }),
            AuthMechanism::XOauth2 => {
                let oauth = self.config.oauth.as_ref().ok_or_else(|| {
                    DeliveryError::permanent(
                        "email transport: xoauth2 configured without oauth settings".to_string(),
                    )
                })?;
                let client_secret = resolve_secret(
                    &self.store,
                    ws,
                    &oauth.client_secret_path,
                    &oauth.client_secret_env,
                )
                .await
                .unwrap_or_default();
                let request = RefreshRequest {
                    token_endpoint: oauth.token_endpoint.clone(),
                    client_id: oauth.client_id.clone(),
                    client_secret,
                    // The sealed value for xoauth2 IS the refresh token (the skill doc's ceremony).
                    refresh_token: secret,
                };
                let token = access_token(&self.tokens, &self.http, &request)
                    .await
                    .map_err(mail_error_to_delivery)?;
                Ok(MailCredentials::XOauth2 {
                    username: self.config.username.clone(),
                    access_token: token,
                })
            }
        }
    }

    fn endpoint(&self) -> SmtpEndpoint {
        SmtpEndpoint {
            host: self.config.host.clone(),
            port: self.config.port,
            tls: self.config.tls,
            timeout: self.config.timeout,
            envelope_from: self.config.envelope_from.clone(),
            allow_invalid_certs: self.config.allow_invalid_certs,
        }
    }
}

#[async_trait]
impl EmailProvider for SmtpEmailProvider {
    async fn send(&self, message: &EmailMessage, meta: &EmailMeta) -> Result<(), DeliveryError> {
        if meta.workspace.trim().is_empty() {
            // The target already refuses this, but the provider must not resolve a secret from a
            // guessed workspace even if some future caller forgets (rule 6, defence in depth).
            return Err(DeliveryError::permanent(
                "email transport: no workspace on the effect — refusing to resolve a credential"
                    .to_string(),
            ));
        }
        let credentials = self.credentials(&meta.workspace).await?;
        let mail = MailMessage {
            from_name: self.config.from_name.clone(),
            from_addr: self.config.from_addr.clone(),
            to: message.to.clone(),
            reply_to: self.config.reply_to.clone(),
            subject: message.subject.clone(),
            text: message.text.clone(),
            html: message.html.clone(),
            message_id: message.message_id.clone(),
            attachments: Vec::new(),
        };

        match send_smtp(&self.endpoint(), &credentials, &mail).await {
            Ok(()) => {
                tracing::info!(
                    to = %message.to, ws = %meta.workspace, host = %self.config.host,
                    "email sent"
                );
                Ok(())
            }
            Err(error) => {
                // An auth rejection can mean a stale access token the relay disagrees with — drop the
                // cached one so the next attempt re-mints rather than replaying the same dead bearer.
                if error.is_permanent() && error.message().contains("auth") {
                    if let Some(oauth) = self.config.oauth.as_ref() {
                        self.tokens.invalidate(&RefreshRequest {
                            token_endpoint: oauth.token_endpoint.clone(),
                            client_id: oauth.client_id.clone(),
                            client_secret: String::new(),
                            refresh_token: resolve_secret(
                                &self.store,
                                &meta.workspace,
                                &self.config.secret_path,
                                &self.config.secret_env,
                            )
                            .await
                            .unwrap_or_default(),
                        });
                    }
                }
                Err(mail_error_to_delivery(error))
            }
        }
    }
}

/// **Sealed workspace secret → node env → unset**, mirroring `agent/resolve_key.rs`.
///
/// The sealed read goes through `lb_secrets::get_workspace` — workspace-walled and
/// `Workspace`-visibility only — because the relay reactor is host machinery with no user principal to
/// carry a `secret:<path>:get` capability. That is the same host-mediated path the agent's model key
/// uses, and it widens no user authority: a ws-B effect can never name ws-A's path, and a `Private`
/// secret never resolves this way.
pub(super) async fn resolve_secret(
    store: &Store,
    ws: &str,
    path: &str,
    env_name: &str,
) -> Option<String> {
    if let Some(path) = Some(path.trim()).filter(|p| !p.is_empty()) {
        if let Ok(value) = lb_secrets::get_workspace(store, ws, path).await {
            if !value.is_empty() {
                return Some(value);
            }
        }
    }
    if let Some(name) = Some(env_name.trim()).filter(|n| !n.is_empty()) {
        if let Ok(value) = std::env::var(name) {
            if !value.is_empty() {
                return Some(value);
            }
        }
    }
    None
}

/// Carry the transport's classification through to the outbox unchanged — the whole point of both types
/// being a two-case split. The message is already sanitized by `lb-mail`.
fn mail_error_to_delivery(error: MailError) -> DeliveryError {
    if error.is_permanent() {
        DeliveryError::permanent(error.message().to_string())
    } else {
        DeliveryError::transient(error.message().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_refuses_a_transport_that_cannot_send() {
        let mut config = SmtpTransportConfig {
            host: "smtp.acme.com".into(),
            from_addr: "reports@acme.com".into(),
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
            username: "reports@acme.com".into(),
            secret_path: "mail/gmail-refresh-token".into(),
            secret_env: "LB_MAIL_SECRET".into(),
            from_addr: "reports@acme.com".into(),
            ..Default::default()
        };
        // Names only — the struct is Debug-safe BY CONSTRUCTION, which is what makes it loggable in a
        // boot diagnostic. (The values live in secrets and are resolved per send.)
        let debug = format!("{config:?}");
        assert!(debug.contains("mail/gmail-refresh-token"));
        assert!(debug.contains("LB_MAIL_SECRET"));
    }

    #[tokio::test]
    async fn an_unresolvable_credential_fails_permanently_and_names_only_the_path() {
        let store = Store::memory().await.unwrap();
        let provider = SmtpEmailProvider::new(
            SmtpTransportConfig {
                host: "127.0.0.1".into(),
                from_addr: "reports@acme.com".into(),
                secret_path: "mail/absent".into(),
                ..Default::default()
            },
            store,
        );
        let err = provider
            .send(
                &EmailMessage {
                    to: "sam@example.com".into(),
                    subject: "s".into(),
                    text: "t".into(),
                    ..Default::default()
                },
                &EmailMeta {
                    workspace: "acme".into(),
                    action: "send_invite".into(),
                },
            )
            .await
            .expect_err("an absent credential must fail the send");
        assert!(err.permanent, "{err}");
        assert!(err.reason.contains("mail/absent"), "{err}");
    }

    #[tokio::test]
    async fn a_send_with_no_workspace_never_resolves_a_credential() {
        let store = Store::memory().await.unwrap();
        let provider = SmtpEmailProvider::new(SmtpTransportConfig::default(), store);
        let err = provider
            .send(
                &EmailMessage {
                    to: "sam@example.com".into(),
                    ..Default::default()
                },
                &EmailMeta {
                    workspace: String::new(),
                    action: "send_invite".into(),
                },
            )
            .await
            .expect_err("an effect without a workspace must not resolve anything");
        assert!(err.permanent, "{err}");
        assert!(err.reason.contains("workspace"), "{err}");
    }
}
