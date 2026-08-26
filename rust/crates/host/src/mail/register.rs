//! `mail.source.register` / `mail.source.update` — create or amend one watched mailbox.
//!
//! One file for both because they are one write with one difference: **register may not clobber a
//! cursor, and update must not lose one.** A re-register of an existing id keeps the stored cursor,
//! counters, and owner; only the configuration is replaced. Without that, an operator fixing a typo
//! in the host name would silently re-import the entire mailbox — the kind of "helpful" reset that
//! costs a workspace its storage budget and fills an inbox with duplicates.

use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_mail_source;
use super::error::MailSourceError;
use super::source::MailSource;
use super::store::{read_source, save_source};

/// Register (or re-register) `source` in `ws` as `principal`.
///
/// The caller supplies configuration only: `cursor`, `owner`, `createdTs`, `lastPollTs`,
/// `lastError`, and the counters are **host-owned** and taken from the stored record (or minted
/// here on first registration), never from the request. A caller that could set its own cursor could
/// make a source skip mail, or re-read it.
pub async fn mail_source_register(
    store: &Store,
    principal: &Principal,
    ws: &str,
    mut source: MailSource,
    now: u64,
) -> Result<MailSource, MailSourceError> {
    authorize_mail_source(principal, ws, "register")?;
    source.id = source.id.trim().to_string();
    source.validate()?;

    match read_source(store, ws, &source.id).await? {
        Some(existing) => {
            // Amend: configuration from the request, history from the record.
            source.cursor = existing.cursor;
            source.owner = existing.owner;
            source.created_ts = existing.created_ts;
            source.last_poll_ts = existing.last_poll_ts;
            source.last_error = existing.last_error;
            source.imported = existing.imported;
            source.rejected = existing.rejected;
        }
        None => {
            source.cursor = Default::default();
            source.owner = principal.sub().to_string();
            source.created_ts = now;
            source.last_poll_ts = 0;
            source.last_error = None;
            source.imported = 0;
            source.rejected = 0;
        }
    }

    save_source(store, ws, &source).await?;
    Ok(source)
}
