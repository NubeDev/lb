//! The **receive** half of the transport: pull messages out of a mailbox and normalize them
//! (mail-source scope). The sibling of [`send`](crate::send), sharing this crate's RFC 5322
//! vocabulary so mail parsing lives in exactly one place.
//!
//! Same posture as the send half: **a transport, not a mail service.** Nothing here has a store, a
//! bus, a capability, or a cursor of its own — [`MailboxCursor`] is a *value* the caller persists.
//! Credentials are passed in per fetch and dropped when it returns. That is what makes the whole
//! half exercisable against a **real IMAP server** on its own (`tests/imap_fetch_test.rs`), which is
//! the only way to prove the protocol rather than assert our own recorder (testing §0).
//!
//! [`MailFetch`] is the one contract. IMAP is v1 because it is the protocol a plain mailbox always
//! speaks; a Gmail-API or JMAP adapter slots in behind the same trait without the poller changing —
//! which is the hedge the scope wanted against provider drift (Google has deprecated IMAP access
//! paths repeatedly).

mod cursor;
mod imap;
mod message;
mod parse;
mod socket;

pub use cursor::MailboxCursor;
pub use imap::{ImapEndpoint, ImapFetch};
pub use message::{FetchedMessage, MailAddress, MailAttachment, ParsedMail};
pub use parse::{parse_message, MAX_BODY_BYTES};

use crate::error::MailResult;

/// The default cap on how many messages one poll pass pulls.
///
/// A bound is mandatory, not a nicety: the first poll of a mailbox with 40,000 messages in it would
/// otherwise try to import all of them in one pass — holding every raw message in memory, blocking
/// the reactor tick, and spending a workspace's storage budget before anyone could react. The
/// remainder is not lost; the cursor advances and the next tick continues. This is the same
/// bounded-work discipline the ingest drain learned the hard way (drain-backpressure scope).
pub const DEFAULT_FETCH_LIMIT: usize = 25;

/// One poll pass's result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchBatch {
    /// The mailbox's `UIDVALIDITY` at the moment of this fetch — the generation `messages`' UIDs
    /// belong to. The caller rebases its cursor onto this (see [`MailboxCursor::rebase`]).
    pub uid_validity: u32,
    /// The messages fetched, ascending by UID.
    pub messages: Vec<FetchedMessage>,
    /// `true` when the mailbox held more messages past the cursor than `limit` allowed. The caller
    /// should poll again promptly rather than waiting a full cadence — a backlog drains at
    /// `limit` per tick, and an operator watching a 40k-message first sync needs it to move.
    pub more: bool,
}

impl FetchBatch {
    /// The highest UID in this batch, or `None` when it is empty.
    pub fn highest_uid(&self) -> Option<u32> {
        self.messages.iter().map(|m| m.uid).max()
    }
}

/// Pull new messages out of one mailbox.
///
/// **The contract is "everything after the cursor, up to `limit`, oldest first."** Implementations
/// must not mutate the mailbox — no `\Seen` flag, no move, no delete. Two reasons: the mailbox is
/// very often a human's too (marking their mail read from under them is a visible bug), and an
/// implementation that relied on a flag to remember its place would be keeping durable state in the
/// external system instead of in the cursor the platform owns (rule 4, in spirit).
#[async_trait::async_trait]
pub trait MailFetch: Send + Sync {
    /// Fetch up to `limit` messages with a UID greater than `cursor.last_uid`, in ascending UID
    /// order. A `cursor` from a different UIDVALIDITY generation reads from the start of the current
    /// one (the returned [`FetchBatch::uid_validity`] tells the caller that happened).
    async fn fetch_since(&self, cursor: &MailboxCursor, limit: usize) -> MailResult<FetchBatch>;

    /// A human-readable description of where this fetches from, for logs and the `check` verb.
    /// **Must never contain a credential** — it goes into log lines and MCP replies.
    fn describe(&self) -> String;
}
