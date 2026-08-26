//! `mail.source.list` / `mail.source.get` — the roster.
//!
//! There is nothing to redact: the record holds a secret **path**, never a value
//! ([`MailSource`](super::source::MailSource)). That is the whole point of the path-only posture,
//! and this file is where it pays off — a read verb over credentials-by-reference needs no
//! scrubbing pass to get wrong.

use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_mail_source;
use super::error::MailSourceError;
use super::source::MailSource;
use super::store::{list_sources, read_source};

/// Every mail source in `ws`.
pub async fn mail_source_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Vec<MailSource>, MailSourceError> {
    authorize_mail_source(principal, ws, "list")?;
    Ok(list_sources(store, ws).await?)
}

/// One mail source, by id.
pub async fn mail_source_get(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<MailSource, MailSourceError> {
    authorize_mail_source(principal, ws, "list")?;
    read_source(store, ws, id)
        .await?
        .ok_or(MailSourceError::NotFound)
}
