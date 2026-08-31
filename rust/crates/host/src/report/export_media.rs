//! `report.export` over the **JSON MCP bridge** — the same composition as the gateway route, with
//! **media ids instead of bytes** on both ends.
//!
//! ```text
//! 1. snapshots up   — media.upload_begin → media.chunk_write × n → media.upload_commit  (shipped)
//! 2. compose        — report.export { id, snapshotMediaId }  →  { pdfMediaId, bytes }   (HERE)
//! 3. bytes down     — media.read { id, offset, limit }                                   (shipped)
//! ```
//!
//! **Why this exists at all.** `POST /reports/{id}/export.pdf` authenticates on
//! `Authorization: Bearer` only, and a module-federated extension UI has no bearer token — its
//! `PageBridge` is `{call, setNav}` and the host withholds the credential on purpose. That left
//! extension authors one working move: lift the session token out of the host's `localStorage` and
//! forge the header. It works today, which is the problem — it voids the leash, bypasses the
//! bridge's per-verb scope filter, and breaks the day page extensions move behind an iframe
//! sandbox. This is the same argument, the same caller and the same answer as
//! [`crate::media_read`], whose module doc states it first; a verb on the bridge is the same bytes
//! through the wall that already exists.
//!
//! **Why ids and not bytes, which is measured rather than chosen.** The obvious shape — snapshots
//! in the request, PDF in the reply — does not fit. `/mcp/call` carries a deliberate 2 MiB
//! blast-radius cap against this route's 32 MiB, and the comment beside it refuses exactly the
//! widening this feature would have asked for:
//!
//!   `// ROUTE-scoped (rule 10): /mcp/call keeps its deliberate 2 MiB blast-radius cap.`
//!   — `rust/role/gateway/src/server.rs`
//!
//! Trading ids keeps the shared bridge's blast radius alone (`/packs/upload` is the shipped
//! precedent for the alternative), and it removes a problem rather than moving one: the PDF is
//! composed once and *stored*, so slices 2..n are ordinary `media.read` calls and no byte cache,
//! digest key or TTL-on-a-cache has to exist. Resumability and progress come free on the rhythm the
//! upload half already uses.
//!
//! **The gate is the shipped one, reused rather than restated.** [`report_export`] authorizes
//! `mcp:report.export:call` FIRST and then re-runs `dashboard_get`'s three gates under the same
//! principal; this function calls it and adds no authorization of its own. The one-line
//! `authorize_report` below is that *same* gate run early so no media byte is read on behalf of a
//! caller who was going to be refused — not a second authorization path, and `report_export`
//! remains the authority.
//!
//! **The PDF is stored under the CALLER's authority**, through the ordinary
//! `media_upload_begin`/`chunk_put`/`commit` verbs, so it is gated by `mcp:media.upload:call` — the
//! grant the caller already needed to put the snapshots up. Storing it with the host's own
//! authority would mint a record the caller may not be able to read back (`media_serve` re-checks
//! `store:media/{id}:read` per item) and would be the second authorization path this module
//! exists to avoid.
//!
//! One responsibility: the media-id envelope around [`report_export`].

use lb_auth::Principal;
use lb_store::Store;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;

use super::authorize::authorize_report;
use super::error::ReportError;
use super::export::report_export;
use super::options::ExportOptions;
use super::rendered::RenderedPanel;
use crate::{
    media_chunk_put, media_serve, media_upload_begin, media_upload_commit, MediaError, CHUNK_SIZE,
};

/// The `origin` tag every PDF this verb stores carries.
///
/// `media.upload_begin` already takes an `origin`, so the provenance seam existed — this is the one
/// thing that distinguishes a generated report artifact from a photograph an operator uploaded.
/// **There is no reaping sweep behind it yet**: nothing in the media module or the jobs crate reads
/// `origin`, and `MediaStatus::Archived` (set by `media.delete`) is the whole of the lifecycle. A
/// report exported nightly therefore leaves a PDF per run, and bounding that is named upstream
/// housekeeping rather than something this verb should invent — see the session doc. Tagging the
/// record now is what makes that sweep a query rather than a migration.
pub const REPORT_ORIGIN: &str = "report.export";

/// The composed PDF's mime. Declared here rather than at the call site because it is also what
/// `media.read` will report to the browser, and the client asserts on it.
const PDF_MIME: &str = "application/pdf";

/// One client-captured panel in the uploaded bundle.
///
/// ⚠ **The field names are the WIRE's**, identical to the gateway route's `ExportBody` and to what
/// rubix-ai's `capturePanels` and the kit's `captureReport` both emit. `compose_pages` keys
/// snapshots on `cell.i` and places an honest error tile for any cell it did not receive, so a
/// divergence here does not throw, does not warn and does not fail a build — it produces a PDF full
/// of titled placeholders while every part of the UI reports success.
#[derive(Debug, Deserialize)]
struct Snapshot {
    #[serde(rename = "cellId")]
    cell_id: String,
    /// RAW base64 PNG — no `data:` prefix. A prefixed string decodes to garbage, and garbage is an
    /// error tile.
    png: String,
}

/// The uploaded snapshot bundle: a JSON document, stored as one media record.
///
/// `snapshots` is **required**, deliberately, and this is the one place the two ways of saying
/// "no captures" are kept apart. A caller who means it omits `snapshotMediaId` altogether and gets
/// the report's skeleton. A caller who uploaded a document that does not carry the key has a wire
/// bug — a serialiser that renamed the field, a half-written blob, the wrong media id — and
/// defaulting it to empty would answer that with a plausible-looking PDF of error tiles while every
/// part of the UI reported success. `#[serde(default)]` here would be one attribute that turns a
/// fixable client defect into a silently wrong document.
#[derive(Debug, Deserialize)]
struct SnapshotBundle {
    snapshots: Vec<Snapshot>,
}

/// Compose the report-kind dashboard `id` from the snapshot bundle stored at `snapshot_media_id`,
/// store the PDF as a media record, and return `{ pdfMediaId, bytes }`.
///
/// `options` is the SECOND DOOR's copy of the same export contract the route takes — the two must
/// carry it identically or the MCP arm and the HTTP route would compose different documents from the
/// same request, which is exactly the drift the byte-identical test exists to catch.
///
/// `snapshot_media_id` may be absent (`None`), which composes the report with **no** captures at
/// all — every cell gets its titled error tile and the page count is unchanged. That is a
/// deliberate, useful shape: it is how a caller renders the report's skeleton, and it is what a
/// scheduled run that could not reach a browser produces. It is never a silent success.
pub async fn report_export_media(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    snapshot_media_id: Option<&str>,
    options: &ExportOptions,
    now: u64,
) -> Result<Value, ReportError> {
    // The SAME gate `report_export` runs first, run early so no media byte is read on behalf of a
    // caller who is about to be refused. `report_export` re-runs it; that is the authority.
    authorize_report(principal, ws, "report.export")?;

    let snapshots = match snapshot_media_id {
        Some(media_id) => read_snapshots(store, principal, ws, media_id).await?,
        None => Vec::new(),
    };

    let pdf = report_export(store, principal, ws, id, snapshots, options, now).await?;

    let pdf_media_id = store_pdf(store, principal, ws, &pdf, now).await?;

    Ok(json!({
        "pdfMediaId": pdf_media_id,
        // The total, so the caller can show a progress readout while it walks the slices down. Named
        // `bytes` to match `media.upload_begin`'s own vocabulary for the same quantity.
        "bytes": pdf.len() as u64,
        "mime": PDF_MIME,
    }))
}

/// Read the uploaded bundle and decode it into what `compose_pages` consumes.
///
/// A snapshot whose base64 does not decode is **refused loudly** rather than skipped. The capture
/// side already skips a block it could not rasterise (an honest gap, placed as a titled tile), so a
/// payload that arrived and did not decode is a wire bug — dropping it silently would turn a
/// fixable client defect into a PDF that looks complete and is not.
async fn read_snapshots(
    store: &Store,
    principal: &Principal,
    ws: &str,
    media_id: &str,
) -> Result<Vec<RenderedPanel>, ReportError> {
    // `media_serve` is the shipped per-item gate (`store:media/{id}:read`) and it refuses anything
    // that is not `Ready` — so a bundle whose upload never committed cannot be composed from.
    let served = media_serve(store, principal, ws, media_id, None)
        .await
        .map_err(media_err)?;

    let bundle: SnapshotBundle = serde_json::from_slice(&served.bytes).map_err(|e| {
        ReportError::BadInput(format!(
            "snapshot media {media_id:?} is not a {{ snapshots: [{{ cellId, png }}] }} document: {e}. \
             To compose with no captures at all, omit `snapshotMediaId` rather than uploading an \
             empty document."
        ))
    })?;

    let mut out = Vec::with_capacity(bundle.snapshots.len());
    for s in bundle.snapshots {
        let bytes = BASE64.decode(s.png.as_bytes()).map_err(|e| {
            ReportError::BadInput(format!(
                "snapshot for cell {:?} is not valid base64 ({e}) — it must be RAW base64 with no `data:` prefix",
                s.cell_id
            ))
        })?;
        // The bundle carries `{ cellId, png }` and no geometry, which is exactly the older wire
        // shape `RenderedPanel` documents: a zero-area rect means "no rendered geometry", so each
        // entry resolves its rect from the record — the shipped layout behaviour, unchanged.
        out.push(RenderedPanel {
            cell_id: s.cell_id,
            png: bytes,
            ..RenderedPanel::default()
        });
    }
    Ok(out)
}

/// Store the composed PDF through the ordinary upload verbs and return its media id.
///
/// begin → chunk_put × n → commit, under the caller's own principal and the caller's own
/// `mcp:media.upload:call` grant. The checksum is computed over the whole document because
/// `media_upload_commit` verifies it — a mismatch there is the store refusing a corrupt artifact
/// loudly rather than serving one silently.
async fn store_pdf(
    store: &Store,
    principal: &Principal,
    ws: &str,
    pdf: &[u8],
    now: u64,
) -> Result<String, ReportError> {
    let checksum = hex_sha256(pdf);

    let begun = media_upload_begin(
        store,
        principal,
        ws,
        PDF_MIME,
        pdf.len() as u64,
        &checksum,
        Some(REPORT_ORIGIN),
        now,
    )
    .await
    .map_err(media_err)?;

    let id = begun
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| ReportError::BadInput("media.upload_begin returned no id".into()))?
        .to_string();

    // `CHUNK_SIZE` is the store's own, read from the module rather than from the reply, so a chunk
    // this writes and a chunk the store expects cannot disagree.
    for (n, chunk) in pdf.chunks(CHUNK_SIZE as usize).enumerate() {
        media_chunk_put(store, principal, ws, &id, n as u32, chunk)
            .await
            .map_err(media_err)?;
    }

    // Verifies every chunk landed and the checksum matches, then flips the record to `Ready`. Until
    // this returns the id is not servable, so it must not be handed back before now.
    media_upload_commit(store, principal, ws, &id, now)
        .await
        .map_err(media_err)?;

    Ok(id)
}

fn hex_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Media failures, in the report service's vocabulary.
///
/// `Denied` and `NotFound` stay **opaque and identical to the report's own**, so a caller cannot
/// use the export verb to probe which media ids exist in a workspace it cannot read.
fn media_err(e: MediaError) -> ReportError {
    match e {
        MediaError::Denied => ReportError::Denied,
        MediaError::NotFound | MediaError::NotReady => ReportError::NotFound,
        MediaError::TooLarge
        | MediaError::BadChecksum
        | MediaError::MissingChunks
        | MediaError::BadInput(_) => ReportError::BadInput(e.to_string()),
        MediaError::Store(s) => ReportError::Media(s),
    }
}
