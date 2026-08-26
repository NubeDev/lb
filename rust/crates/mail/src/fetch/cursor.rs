//! [`MailboxCursor`] — "how far into this mailbox have we already read?", durably.
//!
//! An IMAP UID is only unique **within one UIDVALIDITY generation**. A server that re-creates a
//! mailbox (a restore, a migration, some providers on a whim) bumps `UIDVALIDITY` and re-issues UIDs
//! from 1 — so a cursor that remembered only "last UID 4200" would silently skip the whole new
//! mailbox until it grew past 4200. Both halves therefore travel together, always, and there is no
//! constructor that lets you forget one.
//!
//! **A UIDVALIDITY change resets the cursor; it does NOT re-import the mailbox.** The reset is what
//! makes the poller correct again (it starts reading the new generation from the beginning); the
//! *import ledger* is what stops the re-read turning into duplicate items — every message is still
//! keyed by its `Message-ID` (or its content hash), which does not change when the server renumbers.
//! That two-mechanism split is deliberate: the cursor is an optimization, the ledger is the
//! correctness guarantee. If you ever find yourself relying on the cursor alone for dedup, that is
//! the bug this note exists to prevent.

use serde::{Deserialize, Serialize};

/// How far a poller has read into one mailbox. Durable state, owned by the caller (this crate has no
/// store); `Default` is "never polled", which reads the mailbox from the beginning.
// camelCase on the wire, like every other record this platform serializes. Aliases keep the
// snake_case spelling readable, so a cursor written before this rename still loads (it shipped in
// one live session, and a cursor that silently reads back as `{0, 0}` re-imports a whole mailbox).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailboxCursor {
    /// The `UIDVALIDITY` generation `last_uid` belongs to. `0` = never polled.
    #[serde(default, alias = "uid_validity")]
    pub uid_validity: u32,
    /// The highest UID already imported within that generation. `0` = nothing imported yet.
    #[serde(default, alias = "last_uid")]
    pub last_uid: u32,
}

impl MailboxCursor {
    pub fn new(uid_validity: u32, last_uid: u32) -> Self {
        Self {
            uid_validity,
            last_uid,
        }
    }

    /// Has this mailbox ever been polled?
    pub fn is_fresh(&self) -> bool {
        self.uid_validity == 0
    }

    /// The cursor to use against a mailbox now reporting `uid_validity`.
    ///
    /// Same generation ⇒ unchanged. A **different** generation ⇒ start of that generation (see the
    /// module note: the ledger, not the cursor, prevents the duplicate import that would otherwise
    /// follow). Returns the new cursor and whether a reset happened, so the caller can log it — a
    /// silent reset is how "the poller re-read 900 messages" becomes unexplainable.
    pub fn rebase(&self, uid_validity: u32) -> (Self, bool) {
        if self.uid_validity == uid_validity {
            (*self, false)
        } else {
            (Self::new(uid_validity, 0), !self.is_fresh())
        }
    }

    /// The IMAP UID range to ask for: everything after `last_uid`.
    pub fn uid_range(&self) -> String {
        format!("{}:*", self.last_uid.saturating_add(1))
    }

    /// Advance past `uid` (never backwards — an out-of-order server response must not rewind a
    /// cursor and cause a re-read).
    pub fn advance_to(&mut self, uid: u32) {
        self.last_uid = self.last_uid.max(uid);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wire_shape_is_camel_case_and_still_reads_the_old_snake_case() {
        let cursor = MailboxCursor::new(42, 4200);
        let json = serde_json::to_value(cursor).expect("serialize");
        assert_eq!(json["uidValidity"], 42);
        assert_eq!(json["lastUid"], 4200);
        // A record written by the pre-rename build must not read back as a fresh cursor — that
        // would re-import the entire mailbox on the next poll.
        let legacy: MailboxCursor =
            serde_json::from_str(r#"{"uid_validity":42,"last_uid":4200}"#).expect("legacy");
        assert_eq!(legacy, cursor);
    }

    #[test]
    fn a_fresh_cursor_reads_from_the_start() {
        let c = MailboxCursor::default();
        assert!(c.is_fresh());
        assert_eq!(c.uid_range(), "1:*");
    }

    #[test]
    fn the_same_generation_is_left_alone() {
        let c = MailboxCursor::new(42, 4200);
        let (rebased, reset) = c.rebase(42);
        assert_eq!(rebased, c);
        assert!(!reset);
        assert_eq!(rebased.uid_range(), "4201:*");
    }

    #[test]
    fn a_new_generation_resets_and_reports_it() {
        let c = MailboxCursor::new(42, 4200);
        let (rebased, reset) = c.rebase(43);
        assert_eq!(rebased, MailboxCursor::new(43, 0));
        assert!(
            reset,
            "an operator must be able to see why the mailbox was re-read"
        );
        assert_eq!(rebased.uid_range(), "1:*");
    }

    #[test]
    fn the_first_poll_is_not_reported_as_a_reset() {
        let (rebased, reset) = MailboxCursor::default().rebase(7);
        assert_eq!(rebased, MailboxCursor::new(7, 0));
        assert!(!reset, "never having polled is not a UIDVALIDITY change");
    }

    #[test]
    fn advancing_never_goes_backwards() {
        let mut c = MailboxCursor::new(1, 10);
        c.advance_to(4);
        assert_eq!(
            c.last_uid, 10,
            "an out-of-order uid must not rewind the cursor"
        );
        c.advance_to(11);
        assert_eq!(c.last_uid, 11);
    }
}
