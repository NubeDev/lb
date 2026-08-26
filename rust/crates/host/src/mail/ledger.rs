//! The per-source **import ledger** — the record of every message this source has already seen.
//!
//! ### Why this and not just the cursor
//!
//! The cursor is an optimization ("start reading at UID 4201"); the ledger is the correctness
//! guarantee ("this message is already in"). They fail in different directions and you need both:
//!
//! - A **UIDVALIDITY bump** renumbers the mailbox from 1, so the cursor resets and the whole mailbox
//!   is re-read. Without the ledger that is a full duplicate import.
//! - A **crash between the import and the cursor write** re-reads the last batch. Without the
//!   ledger, duplicates.
//! - A provider **re-delivering** a message at a new UID (some do, after a move) looks brand new to
//!   the cursor. The ledger catches it because the key is derived from the *message*.
//!
//! ### The key
//!
//! `Message-ID` when the message has one (stable across re-delivery, renumbering, and moves), else a
//! digest of the raw bytes. The fallback is not a nicety — plenty of real, machine-generated mail
//! carries no `Message-ID` at all, and keying such a message on its UID would re-import it after
//! every renumber. Both are hashed to a fixed-length hex id so a hostile 4 KB `Message-ID` cannot
//! become a 4 KB record key.
//!
//! ### What a row means
//!
//! A row exists ⇒ **this message will never be imported again**, whatever its status. `Rejected`
//! (the sender was not on the allowlist) is a row for exactly that reason: without it, a rejected
//! sender's message would be re-evaluated on every cursor reset, and an operator widening the
//! allowlist later would get a surprise backfill. The row carries what was decided and why, so the
//! rejection is auditable — the scope wanted quarantine for audit; this keeps the audit trail
//! without spending the workspace's storage on unwanted mail.

use lb_store::{read, write, Store, StoreError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// The store table. Reserved (host-owned): a forged ledger row would make a real message
/// permanently un-importable.
pub const MAIL_IMPORT_TABLE: &str = "mail_import";

/// What happened to one message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImportStatus {
    /// Stored, projected to the inbox, and (where applicable) decoded into series.
    Imported,
    /// The sender was not on the source's allowlist. Nothing was stored.
    Rejected,
    /// Normalization failed. The raw message IS stored (that is the containment rule), so the row
    /// can be re-run after a parser fix by deleting it.
    Failed,
}

/// One ledger row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportRecord {
    /// `{source_id}__{key}` — the record id, repeated in the body so a scan can read it.
    pub id: String,
    pub source: String,
    /// The stable message key (see the module note).
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
    pub uid: u32,
    pub status: ImportStatus,
    #[serde(default)]
    pub from: String,
    #[serde(default)]
    pub subject: String,
    /// The inbox item this message became.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_id: Option<String>,
    /// Asset ids: the raw message first, then one per stored attachment.
    #[serde(default)]
    pub assets: Vec<String>,
    /// Series the attachments decoded into.
    #[serde(default)]
    pub series: Vec<String>,
    #[serde(default)]
    pub samples: usize,
    /// Decode warnings + the rejection/failure reason. Bounded — see [`MAX_LEDGER_NOTES`].
    #[serde(default)]
    pub notes: Vec<String>,
    pub ts: u64,
}

/// The cap on notes kept per row. A pathological file can produce one warning per row for a million
/// rows; storing them all would make the ledger row larger than the data it describes.
pub const MAX_LEDGER_NOTES: usize = 20;

/// The stable, bounded key for a message: its `Message-ID` if it has one, else a digest of the raw
/// bytes. Hashed either way, so the key is fixed-length regardless of what a sender put in a header.
pub fn message_key(message_id: Option<&str>, raw: &[u8]) -> String {
    let mut hasher = Sha256::new();
    match message_id.map(str::trim).filter(|m| !m.is_empty()) {
        Some(id) => {
            // Domain-separated so a message whose Message-ID happens to be the hex of another
            // message's body digest cannot collide with it.
            hasher.update(b"mid:");
            hasher.update(id.to_ascii_lowercase().as_bytes());
        }
        None => {
            hasher.update(b"raw:");
            hasher.update(raw);
        }
    }
    hex(&hasher.finalize())
}

/// The ledger record id for `(source, key)`.
pub fn ledger_id(source: &str, key: &str) -> String {
    format!("{source}__{key}")
}

/// Has this source already handled this message?
pub async fn already_imported(
    store: &Store,
    ws: &str,
    source: &str,
    key: &str,
) -> Result<bool, StoreError> {
    let id = ledger_id(source, key);
    Ok(read(store, ws, MAIL_IMPORT_TABLE, &id).await?.is_some())
}

/// Record the outcome. Idempotent on `(source, key)`.
pub async fn record_import(
    store: &Store,
    ws: &str,
    mut entry: ImportRecord,
) -> Result<(), StoreError> {
    entry.id = ledger_id(&entry.source, &entry.key);
    entry.notes.truncate(MAX_LEDGER_NOTES);
    let value: Value =
        serde_json::to_value(&entry).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, MAIL_IMPORT_TABLE, &entry.id, &value).await
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_message_id_always_yields_the_same_key() {
        let a = message_key(Some("<abc@example.com>"), b"body one");
        let b = message_key(Some("<abc@example.com>"), b"completely different body");
        assert_eq!(a, b, "the key must follow the message, not the octets");
    }

    #[test]
    fn a_message_id_is_case_insensitive_the_way_a_domain_is() {
        assert_eq!(
            message_key(Some("<ABC@Example.COM>"), b""),
            message_key(Some("<abc@example.com>"), b"")
        );
    }

    #[test]
    fn a_message_with_no_id_falls_back_to_its_bytes() {
        let a = message_key(None, b"the same bytes");
        let b = message_key(None, b"the same bytes");
        let c = message_key(None, b"different bytes");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn the_two_key_spaces_are_domain_separated() {
        // A `Message-ID` that is literally the digest text of some body must not collide with it.
        let by_id = message_key(Some("x"), b"");
        let by_raw = message_key(None, b"x");
        assert_ne!(by_id, by_raw);
    }

    #[test]
    fn a_hostile_message_id_cannot_become_a_giant_record_key() {
        let huge = format!("<{}>", "a".repeat(100_000));
        assert_eq!(message_key(Some(&huge), b"").len(), 64);
    }
}
