//! The **mail source** host service — inbound email as a generic producer (mail-source scope).
//!
//! Email is how a very large amount of real-world data actually arrives at a business: reports,
//! invoices, statements, meter exports. The platform could *send* mail (the outbox's `EmailTarget`
//! and the SMTP/Postmark transport) and could not receive any. Any product wanting "email your data
//! in" had to hand-roll IMAP, credential custody, a cursor, and dedup — the exact machinery that
//! should exist once.
//!
//! ```text
//!   a 3rd party sends mail  →  IMAP mailbox
//!                                  │  poll (reactor, per-source cadence)
//!                                  ▼
//!                          lb_mail::fetch  ──►  raw RFC 822 octets
//!                                  │  parse
//!                                  ▼
//!            allowlist → asset(raw) → asset(attachments) → lb_ingest::decode → ingest.write
//!                                  │
//!                                  ▼
//!                          inbox item (+ meta) ──►  the lb inbox
//! ```
//!
//! ### What core knows, and what it does not
//!
//! Core knows RFC 5322 and it knows how to turn a file into samples. It does **not** know what a
//! message means: which tags it should carry, which product it belongs to, what to do about it. That
//! is caller configuration — a rule reacting to the inbox item, an extension verb — never a schema
//! here (rule 10). The one place a file format becomes a code path is `lb_ingest`'s `decode/`
//! registry, reached through an **opaque format id** this service never branches on.
//!
//! One responsibility per file (FILE-LAYOUT §3):
//!   - `source` — the [`MailSource`] record + validation + the sender allowlist.
//!   - `store` — its durable read/write verbs (raw; run after the gate).
//!   - `authorize` — the `mcp:mail.source.<verb>:call` gate.
//!   - `register` / `list` / `remove` — the gated CRUD verbs.
//!   - `check` — prove credentials without importing.
//!   - `fetcher` — resolve the credential and build a live [`MailFetch`](lb_mail::MailFetch).
//!   - `principal` — the deliberately narrow importer identity.
//!   - `ledger` — the per-message import ledger (dedup + audit).
//!   - `import` — normalize one message into assets + series + an inbox item.
//!   - `attachment_ingest` — the attachment → samples → `ingest.write` service.
//!   - `poll` — one bounded pass over one source.
//!   - `reactor` — the tick that makes any of it run.
//!   - `tool` / `descriptor` — the MCP bridge and the palette entries.

mod attachment_ingest;
mod authorize;
mod check;
mod descriptor;
mod error;
mod fetcher;
mod import;
mod ledger;
mod list;
mod poll;
mod principal;
mod reactor;
mod register;
mod remove;
mod source;
mod store;
mod tool;

pub use attachment_ingest::{ingest_attachment, provenance_labels, IngestOutcome, INGEST_CHUNK};
pub use authorize::authorize_mail_source;
pub use check::{check_source, mail_source_check, CheckResult, MessagePeek};
pub use descriptor::mail_descriptors;
pub use error::MailSourceError;
pub use fetcher::{build_fetcher, http_client, token_cache, POLL_TIMEOUT};
pub use import::{
    attachment_asset_id, import_message, item_id, raw_asset_id, ImportOutcome, MAX_ATTACHMENTS,
};
pub use ledger::{
    already_imported, ledger_id, message_key, record_import, ImportRecord, ImportStatus,
    MAIL_IMPORT_TABLE,
};
pub use list::{mail_source_get, mail_source_list};
pub use poll::{poll_source, PollPass};
pub use principal::{mail_import_caps, mail_import_principal, MAIL_IMPORT_SUB};
pub use reactor::{spawn_mail_reactors, MAIL_BATCH, MAIL_TICK};
pub use register::mail_source_register;
pub use remove::{mail_source_delete, mail_source_pause};
pub use source::{
    AttachmentPolicy, MailSource, OauthConfig, DEFAULT_CHANNEL, DEFAULT_POLL_SECONDS,
    MAIL_SOURCE_TABLE, MIN_POLL_SECONDS,
};
pub use store::{delete_source, list_sources, read_source, save_source};
pub use tool::call_mail_tool;
