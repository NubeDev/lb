//! **Attachment → ingest.** The service the mail source exists to reach: an emailed file becomes
//! series data in the workspace's data plane.
//!
//! ### Where the knowledge lives
//!
//! Nothing here knows what a NEM12 record or a CSV column is. The bytes and a format id go to
//! [`lb_ingest::decode`], which owns the format registry; the samples come back and go through the
//! **gated** [`ingest_write`](crate::ingest::ingest_write) under the importer's narrow principal.
//! So this file is: policy check → decode → chunk → write. The seam is what makes a new format a
//! new file in `lb-ingest`'s `decode/` folder rather than a change here.
//!
//! ### The producer identity
//!
//! `ingest.write` roots the producer at the authenticated principal and lets a caller declare a leaf
//! beneath it, so these samples land under `node:mail/{source}`. That matters for a reason the
//! ingest write path documents at length: `seq` is monotonic per `(series, producer)`, so two
//! sources feeding the same series must be two producers or they share one seq space. Here they also
//! *must not* share it with anything else — a mail import and a live modbus poller writing the same
//! series are genuinely independent streams.
//!
//! ### Chunking
//!
//! A year of 5-minute data on four channels is ~420,000 samples from one attachment. Handing that to
//! one `ingest.write` call would build one enormous transaction and hold the store's write guard for
//! the whole of it — the exact failure mode `store/compaction-write-availability-scope.md` was
//! written about. So the write is chunked, and each chunk is an ordinary bounded call.

use lb_auth::Principal;
use lb_ingest::{decode, DecodeInput, DecodeOptions, Sample};
use lb_mail::MailAttachment;
use lb_store::Store;
use serde_json::{json, Map, Value};

use super::error::MailSourceError;
use super::source::MailSource;

/// Samples per `ingest.write` call. Large enough that a big file is not thousands of round trips,
/// small enough that no single call holds the write guard for long.
pub const INGEST_CHUNK: usize = 2_000;

/// What one attachment became.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct IngestOutcome {
    /// The format that ran (resolved, so `auto` reports what it actually chose).
    pub format: String,
    /// The distinct series written.
    pub series: Vec<String>,
    /// Samples the decoder produced.
    pub decoded: usize,
    /// Samples ingest accepted. Lower than `decoded` only if the workspace's own filters or bounds
    /// discarded some — surfaced rather than smoothed over, because a producer that sees
    /// `decoded: 21120, accepted: 4000` needs to look at its retention policy, not at this code.
    pub accepted: usize,
    /// Decode warnings + a truncation notice.
    pub warnings: Vec<String>,
}

/// Decode `attachment` per `source`'s policy and write the samples.
///
/// `Ok(None)` when the policy says this attachment is not for decoding (the common case for a PDF
/// on a source configured for CSV) — that is not a failure and produces no warning.
///
/// A decode *error* is returned to the caller, which records it on the inbox item and the ledger:
/// the message still imports, the attachment is still stored, and a human can see exactly why the
/// numbers did not appear. Failing the whole message because one attachment would not parse would
/// lose the mail.
pub async fn ingest_attachment(
    store: &Store,
    importer: &Principal,
    ws: &str,
    source: &MailSource,
    attachment: &MailAttachment,
    provenance: &Value,
) -> Result<Option<IngestOutcome>, MailSourceError> {
    let policy = &source.attachments;
    if !policy.decodes(attachment.extension()) {
        return Ok(None);
    }

    let input = DecodeInput::new(&attachment.filename, &attachment.mime, &attachment.bytes);
    let options = DecodeOptions {
        series_prefix: policy.series_prefix.clone(),
        offset_minutes: policy.offset_minutes,
        labels: provenance.clone(),
        max_samples: policy.max_samples,
    };
    let decoded = decode(&policy.format, &input, &options)
        .map_err(|e| MailSourceError::BadInput(e.to_string()))?;

    let mut outcome = IngestOutcome {
        format: decoded.format.clone(),
        series: decoded.series.clone(),
        decoded: decoded.samples.len(),
        accepted: 0,
        warnings: decoded.warnings.clone(),
    };
    if decoded.truncated {
        outcome.warnings.push(format!(
            "the file exceeded the {} sample ceiling and was truncated — raise \
             attachments.maxSamples or split the file",
            options.sample_ceiling()
        ));
    }
    if decoded.samples.is_empty() {
        return Ok(Some(outcome));
    }

    // Declare the source as the producer's sub-namespace beneath `node:mail` (see the module note).
    // `ingest.write` sanitizes and roots it; a caller cannot forge another principal's namespace.
    let leaf = source.id.clone();
    let mut samples: Vec<Sample> = decoded.samples;
    for sample in &mut samples {
        sample.producer = leaf.clone();
    }

    for chunk in samples.chunks(INGEST_CHUNK) {
        let accepted = crate::ingest::ingest_write(store, importer, ws, chunk.to_vec())
            .await
            .map_err(ingest_error)?;
        outcome.accepted += accepted;
    }
    Ok(Some(outcome))
}

/// Provenance labels stamped onto every sample an import writes: where it came from, and which
/// message it arrived in. These are what make "why is this series here?" answerable months later
/// without opening the mailbox.
pub fn provenance_labels(source_id: &str, from: &str, message_key: &str, filename: &str) -> Value {
    let mut labels = Map::new();
    labels.insert("mailSource".into(), json!(source_id));
    if !from.is_empty() {
        labels.insert("mailFrom".into(), json!(from));
    }
    labels.insert("mailMessage".into(), json!(message_key));
    if !filename.is_empty() {
        labels.insert("mailAttachment".into(), json!(filename));
    }
    Value::Object(labels)
}

/// The importer holds `mcp:ingest.write:call` and nothing more, so a `Denied` here means the
/// principal bundle and this call disagree — surfaced as `Denied`, not folded into a generic error,
/// so the deny test can tell the two apart.
fn ingest_error(error: crate::ingest::IngestError) -> MailSourceError {
    match error {
        crate::ingest::IngestError::Denied => MailSourceError::Denied,
        crate::ingest::IngestError::BadInput(m) => MailSourceError::BadInput(m),
        crate::ingest::IngestError::Store(e) => MailSourceError::Store(e),
    }
}
