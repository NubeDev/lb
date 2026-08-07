//! [`MailCredentials`] — the resolved credential VALUE for one send.
//!
//! Constructed by the caller from a `secrets/` read immediately before the send and dropped when it
//! returns: nothing here is cached, stored, or serialized. There is deliberately **no `Debug`
//! implementation** — `#[derive(Debug)]` on a credential is exactly how a password reaches a log
//! line, and the compiler refusing `{:?}` is a stronger guarantee than a review comment.
//!
//! [`MailCredentials::redaction_needles`] is the other half of that hygiene: it yields every form the
//! secret can appear in on the wire — the raw value AND its base64 SASL encodings — so
//! [`crate::error::redact`] can strip a server that echoes the AUTH line it rejected.

use base64::Engine;
use mail_send::Credentials;

/// A resolved credential for one submission. No `Debug`, on purpose (see the module note).
#[derive(Clone, PartialEq, Eq)]
pub enum MailCredentials {
    /// No authentication (an open relay).
    None,
    /// Username + password; the SASL mechanism is negotiated from the server's `EHLO`.
    Password { username: String, password: String },
    /// A bearer access token (SASL XOAUTH2), already refreshed.
    XOauth2 {
        username: String,
        access_token: String,
    },
}

impl MailCredentials {
    /// The `mail-send` credential for this value — `None` for an unauthenticated session.
    pub fn to_smtp(&self) -> Option<Credentials<String>> {
        match self {
            MailCredentials::None => None,
            MailCredentials::Password { username, password } => {
                Some(Credentials::new(username.clone(), password.clone()))
            }
            MailCredentials::XOauth2 {
                username,
                access_token,
            } => Some(Credentials::new_xoauth2(
                username.clone(),
                access_token.clone(),
            )),
        }
    }

    /// Every string that must never survive into an error message or a log: the secret itself plus the
    /// exact base64 blobs the SASL mechanisms put on the wire (`\0user\0pass` for PLAIN, the
    /// `user=…\x01auth=Bearer …` frame for XOAUTH2, and the bare-secret LOGIN challenge response).
    pub fn redaction_needles(&self) -> Vec<String> {
        let b64 = base64::engine::general_purpose::STANDARD;
        match self {
            MailCredentials::None => Vec::new(),
            MailCredentials::Password { username, password } => vec![
                password.clone(),
                b64.encode(format!("\u{0}{username}\u{0}{password}")),
                b64.encode(password),
            ],
            MailCredentials::XOauth2 {
                username,
                access_token,
            } => vec![
                access_token.clone(),
                b64.encode(format!(
                    "user={username}\u{1}auth=Bearer {access_token}\u{1}\u{1}"
                )),
            ],
        }
    }

    /// A borrowed view of [`redaction_needles`](Self::redaction_needles) for `redact`.
    pub fn redact_error(&self, message: &str) -> String {
        let needles = self.redaction_needles();
        let refs: Vec<&str> = needles.iter().map(String::as_str).collect();
        crate::error::redact(message, &refs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_needles_cover_the_plain_sasl_blob() {
        let creds = MailCredentials::Password {
            username: "reports@nube.com".into(),
            password: "hunter2hunter2".into(),
        };
        // The exact bytes AUTH PLAIN puts on the wire (see mail-send's `Credentials::encode`).
        let wire = base64::engine::general_purpose::STANDARD
            .encode("\u{0}reports@nube.com\u{0}hunter2hunter2");
        let redacted = creds.redact_error(&format!("535 rejected: AUTH PLAIN {wire}"));
        assert!(!redacted.contains(&wire), "{redacted}");
        assert!(!redacted.contains("hunter2hunter2"), "{redacted}");
        assert!(redacted.contains("535 rejected"), "{redacted}");
    }

    #[test]
    fn xoauth2_needles_cover_the_bearer_frame() {
        let creds = MailCredentials::XOauth2 {
            username: "reports@nube.com".into(),
            access_token: "ya29.a0AfB_verylongtoken".into(),
        };
        let redacted =
            creds.redact_error("334 user=reports@nube.com auth=Bearer ya29.a0AfB_verylongtoken");
        assert!(!redacted.contains("ya29.a0AfB_verylongtoken"), "{redacted}");
    }
}
