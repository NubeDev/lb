//! `lb-mail` — the platform's **mail transport**: build an RFC 5322 message, open a connection,
//! authenticate, put the bytes on the wire (email-transport scope, issue #118).
//!
//! **A transport, not a mail service.** This crate has no store, no bus, no capability logic, and no
//! clock of its own beyond the `Date` header a receiving MTA requires. Durability, retry, backoff and
//! at-least-once belong to the outbox; i18n belongs to `lb_prefs`; routing belongs to the outbox
//! `RouterTarget`'s opaque target string. What was missing — and what lives here — is the one thing
//! nothing else could do: bytes on a wire. Everything is a pure function of its inputs plus one
//! socket, so the whole crate is exercisable against a **real SMTP server** on its own (see
//! `tests/`), which is the only way to prove TLS/auth/MIME rather than asserting a recorder.
//!
//! Folder split (FILE-LAYOUT, folder-of-verbs): [`send`] is the submission half. The **receive** half
//! lands as a sibling `fetch/` when `mail-source-scope.md` builds (IMAP poll → `mail-parser` →
//! normalized message), sharing this crate's address/MIME vocabulary so RFC 5322 lives in one place.
//!
//! Credentials are **values passed in at send time**, never held: a [`MailCredentials`] is built by
//! the caller from a `secrets/` read and dropped when the send returns. This crate never reads a
//! secret store, never logs a credential, and runs every error string through [`redact`] before
//! returning it — a mail library is chatty on failure and an unsanitized SMTP transcript in a log is
//! a credential disclosure.

pub mod error;
pub mod send;

pub use error::{redact, MailError, MailResult};
pub use send::{
    access_token, send_smtp, AuthMechanism, MailCredentials, MailMessage, SmtpEndpoint, TlsMode,
    TokenCache,
};
