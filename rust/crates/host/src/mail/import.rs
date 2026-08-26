//! **Import one message.** The normalization the mail-source scope describes, in the order that
//! makes it recoverable.
//!
//! ```text
//!   allowlist  →  raw message → asset   →  attachments → assets  →  decode → ingest
//!                                                                        ↓
//!                                                        inbox item  ←  summary
//!                                                             ↓
//!                                                        ledger row
//! ```
//!
//! ### Order is the containment strategy
//!
//! **The raw message is stored first, before anything tries to understand it.** Email is a swamp —
//! encodings, malformed MIME, HTML soup — and a normalization step that failed *before* the original
//! was durable would lose mail permanently. Stored first, a parser bug becomes a `failed` ledger row
//! that can be re-run after a fix by deleting it. Nothing later in this file can lose a message.
//!
//! ### The ledger row is written LAST, and on every path
//!
//! Including rejection and failure — see [`ledger`](super::ledger). Writing it last means a crash
//! mid-import re-imports the message, which the idempotent keys make harmless: the raw asset id, the
//! attachment asset ids, the inbox item id, and every sample's `(series, seq)` are all derived from
//! the message key, so a second pass upserts exactly the same rows. What it must never do is write
//! the ledger row first and then crash — that would mark a message imported that is not.
//!
//! ### One message, one inbox item
//!
//! The scope rejected "mail → inbox `Item`s only" because a chat-shaped item loses the attachments.
//! It is right, and the answer is not to skip the item: the item is the **notification** and the
//! assets/series are the payload, joined by `Item.meta`. That is what the meta field was added for.

use lb_auth::Principal;
use lb_mail::{parse_message, FetchedMessage};
use lb_store::Store;
use serde_json::{json, Value};

use super::attachment_ingest::{ingest_attachment, provenance_labels, IngestOutcome};
use super::error::MailSourceError;
use super::ledger::{already_imported, message_key, record_import, ImportRecord, ImportStatus};
use super::source::MailSource;

/// The cap on attachments imported from one message. A hostile sender can attach a thousand parts;
/// each one costs an asset write and a decode attempt. Bounded work per message, always.
pub const MAX_ATTACHMENTS: usize = 20;

/// How many characters of the body ride on the inbox item. The item is a notification, not the
/// document; the full body is in the raw message asset.
pub const BODY_PREVIEW_CHARS: usize = 400;

/// What one message became.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportOutcome {
    pub key: String,
    pub status: ImportStatus,
    pub item_id: Option<String>,
    pub assets: Vec<String>,
    pub series: Vec<String>,
    pub samples: usize,
    pub notes: Vec<String>,
}

impl ImportOutcome {
    fn skipped(key: String, status: ImportStatus, note: String) -> Self {
        Self {
            key,
            status,
            item_id: None,
            assets: Vec::new(),
            series: Vec::new(),
            samples: 0,
            notes: vec![note],
        }
    }
}

/// Import `fetched` for `source`, or report why it was not imported.
///
/// Returns `Ok(None)` when the ledger already has this message — the re-delivery no-op, and the
/// cheapest possible answer (one point read, no parse, no write).
pub async fn import_message(
    store: &Store,
    importer: &Principal,
    ws: &str,
    source: &MailSource,
    fetched: &FetchedMessage,
    now: u64,
) -> Result<Option<ImportOutcome>, MailSourceError> {
    let mail = parse_message(&fetched.raw);
    let key = message_key(mail.message_id.as_deref(), &fetched.raw);
    if already_imported(store, ws, &source.id, &key).await? {
        return Ok(None);
    }

    let from = mail.from_address().to_string();
    let mut outcome = if source.sender_allowed(&from) {
        do_import(store, importer, ws, source, fetched, &mail, &key, now).await
    } else {
        // Rejected: nothing stored, but a row so the decision is auditable and is never re-made.
        Ok(ImportOutcome::skipped(
            key.clone(),
            ImportStatus::Rejected,
            format!("sender '{from}' is not on this source's allowlist"),
        ))
    }
    .unwrap_or_else(|error| {
        // A failure AFTER the raw message was stored is recorded, not propagated: the message must
        // not be retried forever, and the operator needs the reason on the row.
        ImportOutcome::skipped(
            key.clone(),
            ImportStatus::Failed,
            format!("import failed: {error}"),
        )
    });
    outcome.key = key.clone();

    record_import(
        store,
        ws,
        ImportRecord {
            id: String::new(), // derived by `record_import`
            source: source.id.clone(),
            key,
            message_id: mail.message_id.clone(),
            uid: fetched.uid,
            status: outcome.status,
            from,
            subject: mail.subject.clone(),
            item_id: outcome.item_id.clone(),
            assets: outcome.assets.clone(),
            series: outcome.series.clone(),
            samples: outcome.samples,
            notes: outcome.notes.clone(),
            ts: now,
        },
    )
    .await?;

    Ok(Some(outcome))
}

/// The happy path, factored out so the ledger row above is written on every outcome.
#[allow(clippy::too_many_arguments)]
async fn do_import(
    store: &Store,
    importer: &Principal,
    ws: &str,
    source: &MailSource,
    fetched: &FetchedMessage,
    mail: &lb_mail::ParsedMail,
    key: &str,
    now: u64,
) -> Result<ImportOutcome, MailSourceError> {
    let mut outcome = ImportOutcome {
        key: key.to_string(),
        status: ImportStatus::Imported,
        item_id: None,
        assets: Vec::new(),
        series: Vec::new(),
        samples: 0,
        notes: Vec::new(),
    };

    // 1. The immutable original, FIRST (see the module note).
    let raw_id = raw_asset_id(&source.id, key);
    let mut raw_stored = false;
    match crate::assets::put_asset(
        store,
        importer,
        ws,
        &raw_id,
        "message/rfc822",
        fetched.raw.clone(),
        now,
    )
    .await
    {
        Ok(_) => {
            raw_stored = true;
            outcome.assets.push(raw_id.clone());
        }
        // An over-bound message (a 20 MB attachment) must not lose the metadata: the item, the
        // decode, and the ledger row all still happen, with the reason recorded.
        Err(e) => outcome
            .notes
            .push(format!("raw message not stored: {}", asset_reason(e))),
    }

    // 2. Attachments: stored, then decoded.
    let mut attachment_meta = Vec::new();
    for attachment in mail.attachments.iter().take(MAX_ATTACHMENTS) {
        let filename = display_filename(&attachment.filename, attachment_meta.len());
        let asset_id = attachment_asset_id(&source.id, key, attachment_meta.len());
        let mut entry = json!({
            "filename": filename,
            "mime": attachment.mime,
            "bytes": attachment.bytes.len(),
        });

        if source.attachments.store_bytes {
            match crate::assets::put_asset(
                store,
                importer,
                ws,
                &asset_id,
                &attachment.mime,
                attachment.bytes.clone(),
                now,
            )
            .await
            {
                Ok(_) => {
                    outcome.assets.push(asset_id.clone());
                    entry["assetId"] = json!(asset_id);
                }
                Err(e) => outcome.notes.push(format!(
                    "attachment '{filename}' not stored: {}",
                    asset_reason(e)
                )),
            }
        }

        let provenance = provenance_labels(&source.id, mail.from_address(), key, &filename);
        match ingest_attachment(store, importer, ws, source, attachment, &provenance).await {
            Ok(Some(ingested)) => {
                entry["ingest"] = ingest_meta(&ingested);
                outcome.samples += ingested.accepted;
                for series in &ingested.series {
                    if !outcome.series.contains(series) {
                        outcome.series.push(series.clone());
                    }
                }
                outcome.notes.extend(
                    ingested
                        .warnings
                        .into_iter()
                        .map(|w| format!("{filename}: {w}")),
                );
            }
            Ok(None) => {}
            // A file that would not decode is a note on an OTHERWISE SUCCESSFUL import. The bytes
            // are stored; a human can look at them; the mail is not lost.
            Err(e) => {
                entry["ingestError"] = json!(e.to_string());
                outcome
                    .notes
                    .push(format!("{filename}: could not be decoded: {e}"));
            }
        }
        attachment_meta.push(entry);
    }
    if mail.attachments.len() > MAX_ATTACHMENTS {
        outcome.notes.push(format!(
            "message had {} attachments; only the first {MAX_ATTACHMENTS} were imported",
            mail.attachments.len()
        ));
    }

    // 3. The inbox projection — the notification, joined to the payload through `meta`.
    let item_id = item_id(&source.id, key);
    let meta = json!({
        "source": "mail",
        "sourceId": source.id,
        "messageId": mail.message_id,
        "messageKey": key,
        "uid": fetched.uid,
        "from": mail.from,
        "to": mail.to,
        "subject": mail.subject,
        "dateMs": mail.date_ms,
        // `null` when the put failed (an over-bound message), never an id that resolves to
        // nothing — a UI following it would 404 with no explanation, while the reason is right
        // there in `notes`.
        "rawAssetId": raw_stored.then(|| raw_id.clone()),
        "attachments": attachment_meta,
        "series": outcome.series,
        "samples": outcome.samples,
        "notes": outcome.notes,
    });
    crate::inbox::record_inbox_with_meta(
        store,
        importer,
        ws,
        &source.channel,
        &item_id,
        &summary(mail, &outcome),
        // The item's ordering key is the message's OWN date when it has one, so a mailbox imported
        // out of order still reads chronologically; arrival time is the fallback for mail whose
        // `Date` header is missing or nonsense (the header is the sender's clock, and untrusted).
        mail.date_ms.unwrap_or(now),
        meta,
    )
    .await
    .map_err(|_| MailSourceError::Denied)?;
    outcome.item_id = Some(item_id);

    Ok(outcome)
}

/// The one-line body of the inbox item: what arrived, from whom, and what it produced.
fn summary(mail: &lb_mail::ParsedMail, outcome: &ImportOutcome) -> String {
    let subject = if mail.subject.trim().is_empty() {
        "(no subject)"
    } else {
        mail.subject.trim()
    };
    let mut line = format!("{subject} — from {}", mail.from_address());
    if !mail.attachments.is_empty() {
        line.push_str(&format!(", {} attachment(s)", mail.attachments.len()));
    }
    if outcome.samples > 0 {
        line.push_str(&format!(
            ", {} samples into {} series",
            outcome.samples,
            outcome.series.len()
        ));
    }
    let preview: String = mail
        .body()
        .trim()
        .chars()
        .take(BODY_PREVIEW_CHARS)
        .collect();
    if !preview.is_empty() {
        line.push_str("\n\n");
        line.push_str(&preview);
    }
    line
}

fn ingest_meta(outcome: &IngestOutcome) -> Value {
    json!({
        "format": outcome.format,
        "series": outcome.series,
        "decoded": outcome.decoded,
        "accepted": outcome.accepted,
        "warnings": outcome.warnings.len(),
    })
}

/// Ids derived from `(source, message key)` so a re-import upserts the same rows rather than
/// creating a second copy. This is what makes the "ledger last" ordering safe.
pub fn raw_asset_id(source: &str, key: &str) -> String {
    format!("mail-{source}-{key}-raw")
}

pub fn attachment_asset_id(source: &str, key: &str, index: usize) -> String {
    format!("mail-{source}-{key}-att{index}")
}

pub fn item_id(source: &str, key: &str) -> String {
    format!("mail-{source}-{key}")
}

/// A sender-supplied filename is UNTRUSTED text used only as a label. Path separators and control
/// characters are stripped so it can never read as a path in a UI or a log line, and an empty name
/// becomes a positional one rather than a blank.
fn display_filename(raw: &str, index: usize) -> String {
    let cleaned: String = raw
        .chars()
        .filter(|c| !c.is_control() && *c != '/' && *c != '\\')
        .collect();
    let cleaned = cleaned.trim().trim_start_matches('.').to_string();
    if cleaned.is_empty() {
        format!("attachment-{}", index + 1)
    } else {
        cleaned
    }
}

/// An asset error's reason, kept short — it goes on a ledger row and an inbox item.
fn asset_reason(error: crate::assets::AssetError) -> String {
    error.to_string()
}
