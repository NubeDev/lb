//! One **bounded poll pass** over one source: fetch what is new, import it, advance the cursor.
//!
//! ### The cursor is advanced per message, and only after its ledger row exists
//!
//! Not at the end of the batch. A crash halfway through a 25-message batch would otherwise re-fetch
//! all 25 — harmless (the ledger dedups) but slow, and on a first sync of a large mailbox it turns
//! into a loop that never finishes. Advancing per message means a crash costs at most one message's
//! re-fetch. And advancing *after* the ledger row means the two can never disagree in the dangerous
//! direction (cursor past a message with no ledger row = a message silently skipped for ever).
//!
//! ### Errors do not lose the progress already made
//!
//! A transport failure mid-batch still writes back the cursor for everything imported before it, and
//! records the reason on the source. The next tick resumes from there.
//!
//! ### This is where the poll is BOUNDED
//!
//! `limit` messages per pass, `MAX_ATTACHMENTS` attachments per message, `INGEST_CHUNK` samples per
//! write. Every one of those is a bound the drain-backpressure lesson insisted on: a caller must
//! never be billed for unbounded work discovered at runtime.

use lb_mail::{MailFetch, DEFAULT_FETCH_LIMIT};
use lb_store::Store;

use super::error::MailSourceError;
use super::import::import_message;
use super::ledger::ImportStatus;
use super::source::MailSource;
use super::store::save_source;

/// What one pass did.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PollPass {
    pub source: String,
    /// Messages fetched from the mailbox.
    pub fetched: usize,
    /// Newly imported.
    pub imported: usize,
    /// Already in the ledger (a re-delivery, or a UIDVALIDITY re-read).
    pub duplicates: usize,
    /// Refused by the sender allowlist.
    pub rejected: usize,
    /// Stored but not normalized.
    pub failed: usize,
    pub samples: usize,
    pub series: Vec<String>,
    /// The mailbox held more than `limit` past the cursor — poll again promptly.
    pub more: bool,
    /// The pass's failure, if any. Already redacted by the transport.
    pub error: Option<String>,
}

/// Run one pass for `source` in `ws` using `fetcher`, persisting the advanced cursor and counters.
///
/// Takes the fetcher rather than building it so the caller owns credential resolution (and so a test
/// can drive this against a real IMAP server without a secret store).
pub async fn poll_source(
    store: &Store,
    ws: &str,
    source: &mut MailSource,
    fetcher: &dyn MailFetch,
    limit: usize,
    now: u64,
) -> Result<PollPass, MailSourceError> {
    let importer = super::principal::mail_import_principal(ws);
    let limit = if limit == 0 {
        DEFAULT_FETCH_LIMIT
    } else {
        limit
    };
    let mut pass = PollPass {
        source: source.id.clone(),
        ..Default::default()
    };

    let batch = match fetcher.fetch_since(&source.cursor, limit).await {
        Ok(batch) => batch,
        Err(error) => {
            // A failure to even reach the mailbox leaves the cursor alone and is recorded on the
            // source so the roster shows it. The source is NOT paused: a transient outage must not
            // require an operator to re-enable a working mailbox.
            source.last_poll_ts = now;
            source.last_error = Some(error.message().to_string());
            save_source(store, ws, source).await?;
            return Err(error.into());
        }
    };

    // Rebase before importing: a UIDVALIDITY bump means the UIDs in this batch belong to a new
    // generation, and writing them against the old one would leave a cursor that skips mail.
    let (rebased, reset) = source.cursor.rebase(batch.uid_validity);
    source.cursor = rebased;
    if reset {
        tracing::warn!(
            ws = %ws,
            source = %source.id,
            uid_validity = batch.uid_validity,
            "mail source: mailbox UIDVALIDITY changed — re-reading from the start (the import \
             ledger prevents duplicates)"
        );
    }

    pass.fetched = batch.messages.len();
    pass.more = batch.more;

    for message in &batch.messages {
        match import_message(store, &importer, ws, source, message, now).await {
            Ok(Some(outcome)) => {
                match outcome.status {
                    ImportStatus::Imported => {
                        pass.imported += 1;
                        source.imported += 1;
                    }
                    ImportStatus::Rejected => {
                        pass.rejected += 1;
                        source.rejected += 1;
                    }
                    ImportStatus::Failed => pass.failed += 1,
                }
                pass.samples += outcome.samples;
                for series in outcome.series {
                    if !pass.series.contains(&series) {
                        pass.series.push(series);
                    }
                }
            }
            Ok(None) => pass.duplicates += 1,
            Err(error) => {
                // The message could not even be ledgered (a store failure). Stop here WITHOUT
                // advancing past it, persist the progress made, and let the next tick retry — the
                // one case where re-fetching is the correct answer.
                source.last_poll_ts = now;
                source.last_error = Some(error.to_string());
                save_source(store, ws, source).await?;
                pass.error = Some(error.to_string());
                return Ok(pass);
            }
        }
        // Per message, after its ledger row (see the module note).
        source.cursor.advance_to(message.uid);
    }

    source.last_poll_ts = now;
    source.last_error = None;
    save_source(store, ws, source).await?;
    Ok(pass)
}
