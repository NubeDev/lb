//! `mail.source.delete` / `mail.source.pause` — retiring and quieting a source.
//!
//! **Delete does not cascade the import ledger, and that is a decision.** The ledger is what makes
//! re-delivery a no-op; dropping it with the source means that re-registering the same mailbox
//! (which an operator does routinely — to fix a host name, to move a credential) would re-import
//! every message it had already seen. Imported items, assets, and series also survive: they are the
//! workspace's data, not the source's. What is deleted is the *subscription*.
//!
//! Pause is the reversible half: the record, its cursor, and its history stay; it stops being
//! polled. That is the kill switch for a source that has started importing something it should not.

use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_mail_source;
use super::error::MailSourceError;
use super::source::MailSource;
use super::store::{delete_source, read_source, save_source};

/// Delete the subscription. The ledger and everything imported survive (see the module note).
pub async fn mail_source_delete(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<(), MailSourceError> {
    authorize_mail_source(principal, ws, "delete")?;
    // Refuse an id that is not there, so a typo'd delete says so instead of reporting success.
    read_source(store, ws, id)
        .await?
        .ok_or(MailSourceError::NotFound)?;
    delete_source(store, ws, id).await?;
    Ok(())
}

/// Pause or resume a source.
pub async fn mail_source_pause(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    paused: bool,
) -> Result<MailSource, MailSourceError> {
    authorize_mail_source(principal, ws, "pause")?;
    let mut source = read_source(store, ws, id)
        .await?
        .ok_or(MailSourceError::NotFound)?;
    source.paused = paused;
    save_source(store, ws, &source).await?;
    Ok(source)
}
