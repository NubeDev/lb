//! The **shapes** an email travels in: what the target hands a provider, what rides alongside it, and
//! what the provider's acknowledgement is claiming.
//!
//! Split out of `email_target.rs`, which owns the *behaviour* between an effect row and the wire
//! (fan-out, per-recipient dedup, `Message-ID`). These three are the data that behaviour moves, they
//! are what every provider impl names in its signatures, and they change for different reasons than
//! the fan-out does — so they live in their own file, beside `email_payload.rs` (the opaque payload),
//! `email_content.rs` (the words) and `email_attachment.rs` (the bytes).
//!
//! No transport, no credential, no provider name — same rule-10 posture as its neighbours.

use serde::Deserialize;

use super::email_attachment::EmailAttachment;

/// What an email provider's acknowledgement means. Recorded on the delivered-ledger row so a
/// producer can tell "we mailed them" from "we logged it and dropped it" long after the relay pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    /// Handed to a transport.
    Sent,
    /// Written to the log and dropped (a node with no mailer configured).
    Logged,
}

impl Disposition {
    /// The value stored on the ledger row.
    pub fn as_str(self) -> &'static str {
        match self {
            Disposition::Sent => "sent",
            Disposition::Logged => "logged",
        }
    }
}

/// One outbound message, as the target hands it to a provider.
///
/// The HTML half is why this is a struct rather than the four loose `&str`s it used to be: an HTML mail
/// without a plain-text alternative scores badly with every spam filter and is unreadable in a text
/// client, so the pair travels together and the transport builds `multipart/alternative` from it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EmailMessage {
    /// The recipient address. Exactly one — an effect naming several is fanned out into several
    /// messages, so each has its own delivery outcome and its own ledger row.
    pub to: String,
    pub subject: String,
    /// The plain-text body. Always populated.
    pub text: String,
    /// The optional HTML body (from the catalog's `*_html` key, or authored in the payload).
    pub html: Option<String>,
    /// Files to hang off the message — a scheduled report's PDF, resolved from the asset it names.
    pub attachments: Vec<EmailAttachment>,
    /// A stable `Message-ID` for this effect (WITHOUT angle brackets) — identical across retries, so a
    /// receiving MTA can collapse a duplicate the outbox could not know it sent. A mitigation, not a
    /// guarantee: an MTA may ignore it (see `delivered.rs` for the window that remains).
    pub message_id: Option<String>,
}

/// Metadata passed to the email provider alongside the message.
#[derive(Debug, Clone, Deserialize)]
pub struct EmailMeta {
    pub workspace: String,
    #[serde(default)]
    pub action: String,
}

/// Keep a `Message-ID` local part legal: the dedup key is an effect id like `invite:hash1`, and a raw
/// `:` or space in a `Message-ID` makes the header unparseable for the recipient that is supposed to be
/// deduping on it.
///
/// Lives here rather than in `email_target.rs` because what it produces is a FIELD of
/// [`EmailMessage`] — the shape and the rule that keeps the shape legal belong together.
pub(super) fn sanitize_message_id(dedup_key: &str) -> String {
    dedup_key
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect()
}
