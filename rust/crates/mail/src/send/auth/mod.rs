//! How the submission session proves who it is.
//!
//! [`AuthMechanism`] is the config-facing choice; [`MailCredentials`] is the resolved VALUE handed in
//! at send time and dropped when the send returns. [`plain`] covers password submission, [`xoauth2`]
//! covers the bearer-token world (Gmail / Microsoft 365) and owns the token cache, and [`refresh`]
//! performs the OAuth2 refresh-token exchange that keeps that token fresh.
//!
//! **Why XOAUTH2 is v1 and not a follow-up.** Google has been switching off app passwords /
//! less-secure-app access in increments for a decade, and Microsoft removed basic auth from Exchange
//! Online outright. A `password` field demos fine against a test account and fails for real tenants.
//! And an access token expires in about an hour — so "supports Gmail" without refresh is a config
//! field that breaks an hour after setup. Refresh is therefore part of the mechanism, not an add-on.

pub mod plain;
pub mod refresh;
pub mod xoauth2;

pub use plain::MailCredentials;
pub use refresh::{RefreshRequest, TokenEndpointResponse};
pub use xoauth2::{access_token, TokenCache};

use crate::error::MailError;

/// The configured SMTP auth mechanism.
///
/// `Plain` and `Login` both resolve to a username+password credential: the mechanism actually used is
/// **negotiated** from the server's `EHLO` advertisement (strongest available first), so naming
/// `login` when the relay offers `PLAIN` still authenticates. The distinction is kept because it is
/// what operators read off their provider's setup page.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AuthMechanism {
    /// No authentication — an open LAN relay. Explicit, never a fallback from a failed auth.
    None,
    /// Username + password (SASL PLAIN).
    #[default]
    Plain,
    /// Username + password (SASL LOGIN) — same credential as [`AuthMechanism::Plain`].
    Login,
    /// SASL XOAUTH2 with a bearer access token, refreshed from a stored refresh token.
    XOauth2,
}

impl AuthMechanism {
    /// Parse the config string form. An unknown value is a **permanent** config error — a mailer must
    /// not boot with `auth: "oath2"` and silently authenticate some other way.
    pub fn parse(s: &str) -> Result<Self, MailError> {
        match s.trim().to_ascii_lowercase().as_str() {
            "none" => Ok(AuthMechanism::None),
            "plain" | "" => Ok(AuthMechanism::Plain),
            "login" => Ok(AuthMechanism::Login),
            "xoauth2" | "oauth2" => Ok(AuthMechanism::XOauth2),
            other => Err(MailError::Permanent(format!(
                "mail: unknown auth mechanism '{other}' (expected none | plain | login | xoauth2)"
            ))),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            AuthMechanism::None => "none",
            AuthMechanism::Plain => "plain",
            AuthMechanism::Login => "login",
            AuthMechanism::XOauth2 => "xoauth2",
        }
    }

    /// Does this mechanism need a secret at all? (`false` only for [`AuthMechanism::None`].)
    pub fn needs_secret(self) -> bool {
        !matches!(self, AuthMechanism::None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_refuses_a_typo_permanently() {
        assert_eq!(
            AuthMechanism::parse("xoauth2").unwrap(),
            AuthMechanism::XOauth2
        );
        assert_eq!(AuthMechanism::parse("").unwrap(), AuthMechanism::Plain);
        assert!(AuthMechanism::parse("oath2").unwrap_err().is_permanent());
    }
}
