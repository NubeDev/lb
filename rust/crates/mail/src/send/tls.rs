//! [`TlsMode`] — how the submission socket is protected.
//!
//! Three modes, because real relays are all three: `465` is implicit TLS, `587` is STARTTLS, and a
//! LAN/sidecar relay on `25` is often plaintext with no auth. The mode is **config, never inferred
//! from the port** — inferring would make a typo silently downgrade a connection.
//!
//! There is no "opportunistic STARTTLS" mode on purpose. `Starttls` REQUIRES the upgrade: a server
//! that does not advertise it fails the send rather than continuing in the clear, because the next
//! thing on that socket is an AUTH line. `None` is an explicit operator choice, not an accident.

use crate::error::MailError;

/// How to protect the connection.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TlsMode {
    /// TLS from the first byte (submission port `465`).
    Implicit,
    /// Plaintext connect, then a REQUIRED `STARTTLS` upgrade (submission port `587`). The default:
    /// it is what every hosted relay wants.
    #[default]
    Starttls,
    /// No TLS at all — an explicit choice for a trusted LAN relay. Never a fallback.
    None,
}

impl TlsMode {
    /// Parse the config string form. An unknown value is a **permanent** config error: booting a
    /// mailer with `tls: "tsl"` must not quietly pick a mode.
    pub fn parse(s: &str) -> Result<Self, MailError> {
        match s.trim().to_ascii_lowercase().as_str() {
            "implicit" | "tls" | "ssl" => Ok(TlsMode::Implicit),
            "starttls" | "" => Ok(TlsMode::Starttls),
            "none" | "plain" | "plaintext" => Ok(TlsMode::None),
            other => Err(MailError::Permanent(format!(
                "mail: unknown tls mode '{other}' (expected implicit | starttls | none)"
            ))),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            TlsMode::Implicit => "implicit",
            TlsMode::Starttls => "starttls",
            TlsMode::None => "none",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_covers_the_aliases_and_refuses_a_typo() {
        assert_eq!(TlsMode::parse("implicit").unwrap(), TlsMode::Implicit);
        assert_eq!(TlsMode::parse(" SSL ").unwrap(), TlsMode::Implicit);
        assert_eq!(TlsMode::parse("").unwrap(), TlsMode::Starttls);
        assert_eq!(TlsMode::parse("none").unwrap(), TlsMode::None);
        let err = TlsMode::parse("tsl").unwrap_err();
        assert!(err.is_permanent(), "a typo'd tls mode must not be retried");
    }
}
