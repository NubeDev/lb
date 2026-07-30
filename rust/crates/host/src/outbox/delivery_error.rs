//! [`DeliveryError`] — what a [`Target`](super::Target) says when delivery didn't happen, and whether
//! the relay should ever try again.
//!
//! Before this, `deliver` returned `Result<(), String>` and the relay threw the string away
//! (`Err(_reason)`), so every failure was identical: retry with backoff, five times, then park with no
//! recorded reason. That is wrong in both directions. A `550 no such mailbox` or a revoked OAuth grant
//! cannot be fixed by waiting — retrying it delays the dead-letter row an operator needs to see, and
//! against a rate-limiting relay it spends reputation on a mistake. Meanwhile a real transient failure
//! deserved its backoff but left no note behind about what went wrong.
//!
//! So a target now answers two questions in one value: *what happened* (durable, operator-readable,
//! already sanitized) and *is it worth retrying*. `Transient` is the default — `From<String>` and
//! `From<&str>` both produce it, so an existing target that returns a plain string keeps exactly its
//! old behaviour, and permanence is something a target opts into deliberately.

use std::fmt;

/// A failed delivery attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryError {
    /// The operator-readable reason, recorded on the effect row. **Must be sanitized by the target** —
    /// this text is durable, and a mail library will happily hand you an SMTP transcript containing
    /// the AUTH line.
    pub reason: String,
    /// `true` ⇒ the relay parks the effect immediately (no retry). `false` ⇒ normal backoff + retry.
    pub permanent: bool,
}

impl DeliveryError {
    /// A retryable failure — the outbox's backoff owns it. (The default: see [`From<String>`].)
    pub fn transient(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
            permanent: false,
        }
    }

    /// A terminal failure — the effect is parked now with this reason, and never retried.
    pub fn permanent(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
            permanent: true,
        }
    }
}

impl fmt::Display for DeliveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.permanent {
            write!(f, "permanent: {}", self.reason)
        } else {
            write!(f, "{}", self.reason)
        }
    }
}

impl std::error::Error for DeliveryError {}

/// A bare string is a **transient** failure — the pre-existing behaviour of every target, preserved so
/// that adopting the typed error is a signature change and not a semantic one.
impl From<String> for DeliveryError {
    fn from(reason: String) -> Self {
        Self::transient(reason)
    }
}

impl From<&str> for DeliveryError {
    fn from(reason: &str) -> Self {
        Self::transient(reason)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_string_stays_retryable() {
        // The compatibility guarantee: a target that just `?`s a string into the error must not have
        // silently acquired "never retry this" semantics.
        let err: DeliveryError = "device list failed".to_string().into();
        assert!(!err.permanent);
        assert_eq!(err.to_string(), "device list failed");
    }

    #[test]
    fn permanence_is_visible_in_the_recorded_reason() {
        let err = DeliveryError::permanent("smtp 550: no such mailbox");
        assert!(err.permanent);
        assert!(err.to_string().contains("permanent"));
    }
}
