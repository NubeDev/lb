//! `Secret<T>` — the redaction is a *type*, not a guideline (observability scope, README §6.7).
//! Secret material (a token, a DSN, a credential) is wrapped in `Secret<T>` at the secrets surface;
//! its `Debug`/`Display` render `***`, so a `tracing::info!(?secret)` or any `%s`-style format is
//! structurally incapable of leaking the value. The wrapped value is reachable ONLY through
//! [`reveal`](Secret::reveal), which no telemetry path calls.
//!
//! This is the structural mitigation the planted-value redaction test guards (telemetry-console
//! scope): the secret can never reach a span/log/metric field because the type forbids it.

use std::fmt;

/// A secret-holding newtype. `Debug`/`Display` are `***`; the inner value is reached only via
/// [`reveal`](Secret::reveal).
pub struct Secret<T>(T);

impl<T> Secret<T> {
    /// Wrap a secret value. From here on it formats as `***`.
    pub fn new(value: T) -> Self {
        Self(value)
    }

    /// The ONLY way to read the inner value. Telemetry, audit, and the undo journal never call this;
    /// only the live consumer that needs the real credential (the relay, the federation sidecar).
    pub fn reveal(&self) -> &T {
        &self.0
    }
}

impl<T> fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}

impl<T> fmt::Display for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_and_display_redact_to_three_stars() {
        let s = Secret::new("lbk_live_super_secret_value");
        assert_eq!(format!("{s:?}"), "***");
        assert_eq!(format!("{s}"), "***");
        // The real value is reachable only through reveal — never through formatting.
        assert_eq!(s.reveal(), &"lbk_live_super_secret_value");
    }
}
