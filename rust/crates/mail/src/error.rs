//! [`MailError`] — the transport's **honest outcome**: is this worth retrying or not?
//!
//! This is the whole point of the error type. The bug this crate exists to fix
//! (`LoggingEmailProvider`) was a *silent success*: the outbox acked an email nobody sent. The
//! opposite failure is nearly as bad — retrying a `550 no such mailbox` five times with backoff is a
//! retry storm against a mistake that will never resolve. So every failure is classified exactly once,
//! here, and the outbox does the obvious thing with it:
//!
//! - [`MailError::Transient`] — connection refused, timeout, `4xx`, throttle, a failed token refresh:
//!   the effect stays schedulable and the outbox's existing backoff owns the retry.
//! - [`MailError::Permanent`] — `5xx`, an unparseable/absent recipient, a TLS verification failure, a
//!   misconfigured transport: the effect is failed **without** retry and the reason is recorded on the
//!   row for an operator to read.
//!
//! **TLS verification failure is permanent on purpose** and never silently downgraded to plaintext:
//! quietly retrying a bad certificate over cleartext would put credentials on the wire.
//!
//! [`redact`] is the secret-hygiene seam. `mail-send` renders the server's reply verbatim, and a
//! server may echo the AUTH line it just rejected — so the credential VALUES are redacted out of every
//! message this module produces (tested explicitly; care is not a mechanism).

/// The transport's result type.
pub type MailResult<T> = Result<T, MailError>;

/// A send failure, classified by whether retrying can plausibly help.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MailError {
    /// Retry later — the outbox's backoff owns it (connection, timeout, `4xx`, throttle, refresh).
    #[error("transient: {0}")]
    Transient(String),
    /// Never retry — the effect fails now with this reason (`5xx`, bad recipient, TLS, config).
    #[error("permanent: {0}")]
    Permanent(String),
}

impl MailError {
    /// Is this failure terminal (no retry)?
    pub fn is_permanent(&self) -> bool {
        matches!(self, MailError::Permanent(_))
    }

    /// The message, without the transient/permanent prefix.
    pub fn message(&self) -> &str {
        match self {
            MailError::Transient(m) | MailError::Permanent(m) => m,
        }
    }

    /// Redact `secrets` out of this error's message (see [`redact`]). Applied at every construction
    /// site that can carry server-echoed text.
    pub fn redacted(self, secrets: &[&str]) -> Self {
        match self {
            MailError::Transient(m) => MailError::Transient(redact(&m, secrets)),
            MailError::Permanent(m) => MailError::Permanent(redact(&m, secrets)),
        }
    }
}

/// The redaction marker substituted for a credential value.
pub const REDACTED: &str = "[redacted]";

/// Replace every occurrence of each non-empty `secret` in `text` with [`REDACTED`].
///
/// Used on the way OUT of the transport, on server-supplied text, because an SMTP server may echo the
/// `AUTH PLAIN <base64>` line it rejected — and a base64 blob of `\0user\0password` in a log is a
/// credential disclosure. Callers pass every form the secret can appear in (the raw value AND its
/// base64/SASL encodings); this function is deliberately dumb so there is nothing to get subtly wrong.
///
/// Short needles are skipped (< 4 bytes): redacting a 1-character "secret" would shred the message
/// into noise for no security gain.
pub fn redact(text: &str, secrets: &[&str]) -> String {
    let mut out = text.to_string();
    for secret in secrets {
        if secret.len() < 4 {
            continue;
        }
        if out.contains(secret) {
            out = out.replace(secret, REDACTED);
        }
    }
    out
}

/// Classify an SMTP reply code (`421`, `550`, …). `4xx` is transient, `5xx` is permanent, and
/// anything else unexpected is treated as transient (an unknown reply may be a proxy hiccup — the
/// outbox retrying a few times and then dead-lettering is the safer default than failing outright).
pub fn classify_smtp_code(code: u16, message: impl Into<String>) -> MailError {
    let message = message.into();
    if (500..600).contains(&code) {
        MailError::Permanent(message)
    } else {
        MailError::Transient(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_hundreds_retry_five_hundreds_do_not() {
        assert!(!classify_smtp_code(421, "too many auth attempts").is_permanent());
        assert!(!classify_smtp_code(451, "try again").is_permanent());
        assert!(classify_smtp_code(550, "no such mailbox").is_permanent());
        assert!(classify_smtp_code(552, "over quota").is_permanent());
        // An unknown code retries rather than failing outright.
        assert!(!classify_smtp_code(0, "socket closed").is_permanent());
    }

    #[test]
    fn redact_removes_every_occurrence_and_skips_short_needles() {
        let msg = "535 rejected AUTH PLAIN AHVzZXIAcHc= for hunter2hunter2 (hunter2hunter2)";
        let out = redact(msg, &["hunter2hunter2", "AHVzZXIAcHc=", "a"]);
        assert!(!out.contains("hunter2hunter2"), "{out}");
        assert!(!out.contains("AHVzZXIAcHc="), "{out}");
        // The 1-char needle must NOT have shredded the message.
        assert!(out.contains("535 rejected"), "{out}");
    }
}
