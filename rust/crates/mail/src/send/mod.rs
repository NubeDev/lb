//! The **send** half of the transport: build a message, connect, authenticate, submit.
//!
//! One verb per file (FILE-LAYOUT): [`message`] builds MIME, [`tls`] decides how the socket is
//! protected, [`auth`] decides how the session proves who it is, [`smtp`] runs the session. The
//! receive half lands beside this as `fetch/` (`mail-source-scope.md`).

pub mod auth;
pub mod message;
pub mod smtp;
pub mod tls;

pub use auth::{access_token, AuthMechanism, MailCredentials, TokenCache};
pub use message::MailMessage;
pub use smtp::{send_smtp, SmtpEndpoint};
pub use tls::TlsMode;
