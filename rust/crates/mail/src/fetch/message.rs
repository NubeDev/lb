//! The **receive** vocabulary: the bytes an IMAP server handed back, and the normalized shape a
//! caller actually wants out of them (mail-source scope, "Normalization, receive-only").
//!
//! Two deliberately separate types, because they have different lifetimes and different truth
//! claims:
//!
//! - [`FetchedMessage`] is the **immutable original** — the RFC 822 octets exactly as the server
//!   served them, plus the UID they were served under. Nothing here is interpreted. The mail-source
//!   scope's containment strategy ("the raw message is always stored first — normalization can fail
//!   per-message and be re-run after a parser fix, never losing mail") only works if this type
//!   exists on its own and can be persisted before anything tries to understand it.
//! - [`ParsedMail`] is a **best-effort reading** of those bytes. Email is a swamp: 8-bit subjects,
//!   HTML-only bodies, `Content-Disposition` headers that disagree with `Content-Type`, missing
//!   `Message-ID`s. Every field here is therefore optional or defaulted, and a parse that recovers
//!   only half a message still returns — it never throws the mail away.
//!
//! No store, no bus, no clock: these are plain data (this crate is a transport, `lib.rs`).

use serde::{Deserialize, Serialize};

/// One message as the server served it: the UID it lives at, and its raw RFC 822 octets.
///
/// `uid` is the mailbox-scoped IMAP UID, and is only meaningful **paired with the
/// [`uid_validity`](super::MailboxCursor::uid_validity) it was fetched under — a server that
/// re-creates a mailbox re-issues UIDs from 1, which is precisely why the cursor carries both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchedMessage {
    /// The IMAP UID this message was served at, within its mailbox's current UIDVALIDITY.
    pub uid: u32,
    /// The immutable original: RFC 822 octets, byte-for-byte as received.
    pub raw: Vec<u8>,
}

impl FetchedMessage {
    pub fn new(uid: u32, raw: Vec<u8>) -> Self {
        Self { uid, raw }
    }
}

/// One mailbox address, split into the display name and the addr-spec.
///
/// `address` is lower-cased at parse time so an allowlist comparison is a plain `==` and cannot be
/// defeated by `Alerts@Nube-IO.com` (the sender-allowlist knob the mail-source scope insists ships
/// in v1 — case-folding the *domain* is required by RFC 5321, and folding the local part too is the
/// pragmatic choice every real mail system makes).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MailAddress {
    /// The display name, when the header carried one (`Alerts <alerts@nube-io.com>` → `Alerts`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The addr-spec, lower-cased.
    pub address: String,
}

impl MailAddress {
    pub fn new(name: Option<String>, address: impl AsRef<str>) -> Self {
        Self {
            name: name.filter(|n| !n.trim().is_empty()),
            address: address.as_ref().trim().to_ascii_lowercase(),
        }
    }

    /// The domain half of the addr-spec (`""` when the address is malformed enough to have none).
    pub fn domain(&self) -> &str {
        self.address.split_once('@').map_or("", |(_, d)| d)
    }
}

/// One decoded attachment. `bytes` is the **decoded** payload (base64/quoted-printable already
/// undone), so a caller can hash it, store it, or hand it to a decoder without knowing the transfer
/// encoding it arrived in.
///
/// `filename` may be empty — plenty of real mail attaches a part with no name at all. Callers that
/// need a name must supply their own fallback rather than trusting this to be non-empty, and must
/// treat whatever is here as **untrusted text** (it comes from the sender): it is a label, never a
/// path. Nothing in this crate opens a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MailAttachment {
    /// The sender-declared filename, or `""`. Untrusted; never a path.
    pub filename: String,
    /// The declared content type (`text/csv`, `application/pdf`, …), lower-cased, parameters stripped.
    pub mime: String,
    /// The decoded payload.
    pub bytes: Vec<u8>,
}

impl MailAttachment {
    /// The lower-cased extension of [`filename`](Self::filename), without the dot (`""` when there
    /// is none). The cheapest honest signal about what these bytes are — the declared `mime` is
    /// frequently `application/octet-stream` for a file the sender's client did not recognize.
    pub fn extension(&self) -> &str {
        self.filename
            .rsplit_once('.')
            .map_or("", |(_, ext)| ext)
            .trim()
    }
}

/// A best-effort reading of one message. Every field is optional or defaulted on purpose — see the
/// module doc: a half-parsed message is still delivered, never dropped.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedMail {
    /// The `Message-ID` header, angle brackets stripped. **Absent on real mail more often than you
    /// would like**, which is why the import ledger falls back to a content hash rather than
    /// treating this as a key it can rely on.
    pub message_id: Option<String>,
    /// The `In-Reply-To` header, angle brackets stripped. Carried so a later threading view can be
    /// built without a re-import (threading itself is a mail-source non-goal).
    pub in_reply_to: Option<String>,
    /// The envelope sender as the headers declare it.
    pub from: Option<MailAddress>,
    /// Every `To:` recipient.
    pub to: Vec<MailAddress>,
    /// The decoded subject (`""` when absent — a subject-less message is legal).
    pub subject: String,
    /// The `Date` header as epoch **milliseconds**, when it parsed. Untrusted (the sender's clock);
    /// a caller that needs a trustworthy instant should stamp its own arrival time.
    pub date_ms: Option<u64>,
    /// The `text/plain` body, decoded.
    pub text: Option<String>,
    /// The `text/html` body, decoded.
    pub html: Option<String>,
    /// Every attachment part, decoded.
    pub attachments: Vec<MailAttachment>,
}

impl ParsedMail {
    /// The best available body text: the plain part, else the HTML part. Deliberately NOT an
    /// HTML→markdown conversion — that is the extraction seam's job, and the raw message is the
    /// fidelity escape hatch either way (mail-source scope, "HTML fidelity" non-goal).
    pub fn body(&self) -> &str {
        self.text
            .as_deref()
            .or(self.html.as_deref())
            .unwrap_or_default()
    }

    /// The sender's addr-spec, or `""`.
    pub fn from_address(&self) -> &str {
        self.from.as_ref().map_or("", |f| f.address.as_str())
    }
}
