//! [`send_smtp`] — one SMTP submission: connect, protect, authenticate, `MAIL FROM`/`RCPT TO`/`DATA`.
//!
//! **The timeout is mandatory, not a nicety.** This function is called from inside the outbox relay
//! tick, and an SMTP session can hang for minutes on a half-open socket — which would stall *all*
//! outbox delivery behind it, push notifications included. So [`SmtpEndpoint::timeout`] bounds the
//! whole session and there is no way to construct an endpoint without one.
//!
//! **Every error is classified and redacted before it leaves.** `mail-send` renders the server's reply
//! verbatim and a server may echo the AUTH line it rejected, so the reply text goes through
//! [`MailCredentials::redact_error`] on the way out — the credential values (and their base64 SASL
//! encodings) can never reach an outbox row or a log line. See [`crate::error`] for the
//! transient/permanent split the outbox acts on.

use std::time::Duration;

use mail_send::{Error as SmtpError, SmtpClientBuilder};

use super::auth::MailCredentials;
use super::message::MailMessage;
use super::tls::TlsMode;
use crate::error::{classify_smtp_code, MailError, MailResult};

/// Where and how to submit. Node-level config in v1 (one relay per node, like a system mailer).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmtpEndpoint {
    pub host: String,
    pub port: u16,
    pub tls: TlsMode,
    /// The whole-session bound (see the module note). Never `None`.
    pub timeout: Duration,
    /// The `MAIL FROM` envelope sender when it must differ from the header `From` (bounce handling,
    /// SRS). `None` ⇒ the header `From` address is used.
    pub envelope_from: Option<String>,
    /// Accept an invalid/self-signed server certificate. **Test-only knob**: the in-test TLS server
    /// has no CA-signed cert. A real transport leaves this `false`, and a verification failure is a
    /// permanent error rather than a silent downgrade to cleartext.
    pub allow_invalid_certs: bool,
}

impl SmtpEndpoint {
    /// A STARTTLS endpoint with an explicit timeout.
    pub fn new(host: impl Into<String>, port: u16, tls: TlsMode, timeout: Duration) -> Self {
        Self {
            host: host.into(),
            port,
            tls,
            timeout,
            envelope_from: None,
            allow_invalid_certs: false,
        }
    }
}

/// Submit `message` through `endpoint` as `credentials`.
///
/// `Ok(())` means the relay returned a positive completion for `DATA` — it has taken responsibility
/// for the message. Everything else is a classified [`MailError`].
///
/// **The timeout is enforced HERE, around the whole session**, not delegated to the SMTP client. That
/// is not belt-and-braces: `mail-send` bounds its TLS connect and each `cmd`, but its cleartext
/// `connect_plain` reads the server *greeting* with no bound at all — a server that accepts the socket
/// and then says nothing hangs the caller forever. The transport test that scripts exactly that server
/// caught it (30s hang against a 300ms timeout), and since the caller is the outbox relay tick, a hang
/// here stalls every other outbox delivery. So the bound is ours to guarantee.
pub async fn send_smtp(
    endpoint: &SmtpEndpoint,
    credentials: &MailCredentials,
    message: &MailMessage,
) -> MailResult<()> {
    let timeout = endpoint.timeout;
    tokio::time::timeout(timeout, submit(endpoint, credentials, message))
        .await
        .unwrap_or_else(|_| {
            Err(MailError::Transient(format!(
                "smtp: session exceeded the {}s timeout",
                timeout.as_secs_f32()
            )))
        })
}

/// One submission attempt, unbounded — always called through [`send_smtp`]'s timeout.
async fn submit(
    endpoint: &SmtpEndpoint,
    credentials: &MailCredentials,
    message: &MailMessage,
) -> MailResult<()> {
    let body = message.to_rfc5322()?;
    let envelope_from = endpoint
        .envelope_from
        .clone()
        .unwrap_or_else(|| message.from_addr.clone());

    let mut builder = SmtpClientBuilder::new(endpoint.host.clone(), endpoint.port)
        .map_err(|e| MailError::Permanent(format!("mail: bad smtp host: {e}")))?
        .timeout(endpoint.timeout)
        .implicit_tls(endpoint.tls == TlsMode::Implicit);
    if endpoint.allow_invalid_certs {
        builder = builder.allow_invalid_certs();
    }
    if let Some(creds) = credentials.to_smtp() {
        builder = builder.credentials(creds);
    }

    let envelope =
        mail_send::smtp::message::Message::new(envelope_from, [message.to.clone()], body);

    // `connect` covers implicit TLS AND the REQUIRED STARTTLS upgrade (a server that does not
    // advertise it errors rather than continuing in the clear); `connect_plain` is the explicit
    // no-TLS choice. Both authenticate as part of the connect, so the two arms differ only in the
    // stream type — `submit` below is the one copy of the actual submission.
    match endpoint.tls {
        TlsMode::Implicit | TlsMode::Starttls => {
            let mut client = builder
                .connect()
                .await
                .map_err(|e| classify_smtp_error(e, credentials))?;
            client.send(envelope).await
        }
        TlsMode::None => {
            let mut client = builder
                .connect_plain()
                .await
                .map_err(|e| classify_smtp_error(e, credentials))?;
            client.send(envelope).await
        }
    }
    .map_err(|e| classify_smtp_error(e, credentials))
}

/// Map a `mail-send` error to the transient/permanent split, redacting credential material.
///
/// The mapping is the honest-outcome contract:
/// - a server reply carries its own verdict (`4xx` retry, `5xx` give up) — [`classify_smtp_code`];
/// - an auth failure is permanent **unless** the server said `4xx` (Gmail answers `421 too many auth
///   attempts` under rate-limiting, which is emphatically retryable);
/// - I/O, timeout and a missing STARTTLS advertisement are transient — a relay comes back;
/// - a TLS error is **permanent and loud**: never retried into a cleartext fallback;
/// - a missing credential/sender/recipient is permanent — a config or payload bug.
fn classify_smtp_error(error: SmtpError, credentials: &MailCredentials) -> MailError {
    let classified = match error {
        SmtpError::UnexpectedReply(reply) => classify_smtp_code(
            reply.code(),
            format!("smtp {}: {}", reply.code(), reply.message()),
        ),
        SmtpError::AuthenticationFailed(reply) => classify_smtp_code(
            reply.code(),
            format!("smtp auth failed ({}): {}", reply.code(), reply.message()),
        ),
        SmtpError::Io(e) => MailError::Transient(format!("smtp io: {e}")),
        SmtpError::Timeout => MailError::Transient("smtp: session timed out".into()),
        SmtpError::MissingStartTls => MailError::Transient(
            "smtp: server does not advertise STARTTLS (refusing to send in the clear)".into(),
        ),
        SmtpError::Tls(e) => MailError::Permanent(format!("smtp tls: {e}")),
        SmtpError::InvalidTLSName => {
            MailError::Permanent("smtp: invalid TLS name for the configured host".into())
        }
        SmtpError::MissingCredentials => MailError::Permanent(
            "smtp: server requires authentication but none is configured".into(),
        ),
        SmtpError::UnsupportedAuthMechanism => MailError::Permanent(
            "smtp: server supports none of the configured auth mechanisms".into(),
        ),
        SmtpError::MissingMailFrom => MailError::Permanent("smtp: no envelope sender".into()),
        SmtpError::MissingRcptTo => MailError::Permanent("smtp: no envelope recipient".into()),
        SmtpError::Auth(e) => MailError::Permanent(format!("smtp auth: {e}")),
        SmtpError::Base64(e) => MailError::Permanent(format!("smtp: bad base64 from server: {e}")),
        SmtpError::UnparseableReply => {
            MailError::Transient("smtp: unparseable reply (connection dropped?)".into())
        }
    };
    // Secret hygiene, mechanically: strip the credential VALUES out of whatever the server said.
    let needles = credentials.redaction_needles();
    let refs: Vec<&str> = needles.iter().map(String::as_str).collect();
    classified.redacted(&refs)
}
