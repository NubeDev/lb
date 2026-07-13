//! Derive docs from ONE source media id — the per-item unit of the `docs.extract` job. Everything
//! that can go wrong with a single item is contained HERE and turned into an [`ItemOutcome`], so
//! the batch loop (`extract.rs`) always completes: a corrupt file, an unsupported mime, a denied
//! read, or even a panicking parser fails *this item*, never the job (scope: "panicking extractor
//! contained to its item").
//!
//! The pipeline, in order:
//!   1. read reach — re-gate `media.get` under the caller (deny → `Denied`, no existence leak);
//!   2. load media + bytes (missing/not-ready → `Denied`/`Failed`);
//!   3. pick the extractor for the mime (none → `Unsupported`);
//!   4. ledger check — same `(checksum, version)` → idempotent no-op (`reused: true`);
//!   5. run the extractor, PANIC-CONTAINED (`catch_unwind`); Err → `Unsupported`/`Failed`;
//!   6. write each derived doc through the host `put_doc` (its own `store:doc/*:write` gate + the
//!      markdown link-graph), edge each `derived_from` the media, record the ledger.
//!
//! Derived doc ids are STABLE (`derived_from` `{media}:{extractor}[:{part}]`) so a version bump
//! re-derives into the same ids — links and embeddings migrate instead of orphaning.

use std::panic::{catch_unwind, AssertUnwindSafe};

use lb_assets::{relate, ContentType};
use lb_auth::Principal;
use lb_extract::{extractor_for, ExtractError, ExtractOpts, ExtractedDoc};
use lb_store::Store;

use crate::assets::put_doc;
use crate::media::model::{media_get_raw, read_all_bytes};

use super::authorize::may_read_media;
use super::ledger::{get_extraction, is_fresh, put_extraction};
use super::model::{ExtractRequest, Extraction, ItemOutcome, DERIVED_FROM};

/// Derive docs for one media id. Never returns `Err` for an item-level problem — those are
/// [`ItemOutcome`]s. A `Result::Err` is reserved for a genuine store failure that should surface as
/// a job-level problem (the caller decides; today the loop treats it as `Failed` too).
pub async fn derive_one(
    store: &Store,
    principal: &Principal,
    ws: &str,
    req: &ExtractRequest,
    media_id: &str,
    ts: u64,
) -> ItemOutcome {
    // 1. Per-item read reach — the caller's own `media.get` gate. A deny here is opaque: another
    //    workspace's id, a nonexistent id, and a truly-denied id are indistinguishable.
    if !may_read_media(principal, ws) {
        return ItemOutcome::Denied {
            media_id: media_id.to_string(),
        };
    }

    // 2. Load the media metadata + bytes. Cross-workspace / missing → None (isolation) → Denied.
    let media = match media_get_raw(store, ws, media_id).await {
        Ok(Some(m)) => m,
        Ok(None) => {
            return ItemOutcome::Denied {
                media_id: media_id.to_string(),
            }
        }
        Err(e) => {
            return ItemOutcome::Failed {
                media_id: media_id.to_string(),
                reason: format!("media read error: {e}"),
            }
        }
    };

    // 3. Select the extractor for the mime — none → honest Unsupported (never an empty doc).
    let extractor = match extractor_for(&media.mime) {
        Some(e) => e,
        None => {
            return ItemOutcome::Unsupported {
                media_id: media_id.to_string(),
                reason: format!("no extractor for mime {}", media.mime),
            }
        }
    };

    // 4. Ledger idempotency — same checksum + version floor → no-op, echo the existing doc ids.
    match get_extraction(store, ws, media_id, extractor.id()).await {
        Ok(Some(existing)) if is_fresh(&existing, &media.checksum, req.force_version) => {
            return ItemOutcome::Extracted {
                media_id: media_id.to_string(),
                doc_ids: existing.doc_ids,
                reused: true,
            };
        }
        Ok(_) => {}
        Err(e) => {
            return ItemOutcome::Failed {
                media_id: media_id.to_string(),
                reason: format!("ledger read error: {e}"),
            }
        }
    }

    // 5. Read bytes and run the extractor, contained against a panic (untrusted binary input).
    let bytes = match read_all_bytes(store, ws, &media).await {
        Ok(b) => b,
        Err(e) => {
            return ItemOutcome::Failed {
                media_id: media_id.to_string(),
                reason: format!("cannot read media bytes: {e}"),
            }
        }
    };
    let opts = ExtractOpts {
        split: req.split,
        max_table_cells: 0,
    };
    let extracted = match run_contained(&*extractor, &bytes, &media.mime, &opts) {
        Ok(docs) => docs,
        Err(ExtractError::Unsupported(reason)) => {
            return ItemOutcome::Unsupported {
                media_id: media_id.to_string(),
                reason,
            }
        }
        Err(ExtractError::Failed(reason)) => {
            return ItemOutcome::Failed {
                media_id: media_id.to_string(),
                reason,
            }
        }
    };

    // 6. Write each derived doc (stable id), edge it `derived_from` the media, record the ledger.
    let mut doc_ids = Vec::with_capacity(extracted.len());
    for part in &extracted {
        let doc_id = derived_doc_id(media_id, extractor.id(), part);
        let title = req
            .title
            .clone()
            .filter(|t| !t.is_empty())
            .or_else(|| non_empty(&part.title_hint))
            .unwrap_or_else(|| media_id.to_string());
        if let Err(e) = put_doc(
            store,
            principal,
            ws,
            &doc_id,
            &title,
            &part.markdown,
            ContentType::Markdown,
            &req.tags,
            ts,
        )
        .await
        {
            // A doc-write denial (no `store:doc/*:write`) or store error fails the whole item —
            // the caller lacks doc write, so no part will land. Honest per-item failure.
            return ItemOutcome::Failed {
                media_id: media_id.to_string(),
                reason: format!("derived doc write failed: {e}"),
            };
        }
        // The derivation edge (derived doc → source media). Best-effort like the doc's own link
        // edges: a failed edge does not orphan the successfully-written doc; it reconciles on the
        // next derivation. The doc is the value; the edge is a derived index.
        let _ = relate(store, ws, DERIVED_FROM, &doc_id, media_id).await;
        doc_ids.push(doc_id);
    }

    let rec = Extraction {
        id: Extraction::make_id(media_id, extractor.id()),
        media_id: media_id.to_string(),
        media_checksum: media.checksum.clone(),
        extractor_id: extractor.id().to_string(),
        extractor_version: extractor.version(),
        doc_ids: doc_ids.clone(),
        ts,
    };
    if let Err(e) = put_extraction(store, ws, &rec).await {
        return ItemOutcome::Failed {
            media_id: media_id.to_string(),
            reason: format!("ledger write failed: {e}"),
        };
    }

    ItemOutcome::Extracted {
        media_id: media_id.to_string(),
        doc_ids,
        reused: false,
    }
}

/// Run an extractor with a panic guard. A pure parser over untrusted bytes is the classic attack
/// surface (scope risk); a panic here is caught and reported as `Failed`, containing it to this
/// item (the pure crate also guards its known-panicky PDF path — defense in depth).
fn run_contained(
    extractor: &dyn lb_extract::Extractor,
    bytes: &[u8],
    mime: &str,
    opts: &ExtractOpts,
) -> Result<Vec<ExtractedDoc>, ExtractError> {
    match catch_unwind(AssertUnwindSafe(|| extractor.extract(bytes, mime, opts))) {
        Ok(res) => res,
        Err(_) => Err(ExtractError::failed(
            "extractor panicked (contained to this item)",
        )),
    }
}

/// The stable derived-doc id: `derived_from` `{media}:{extractor}` for a whole-source doc, plus
/// `:{part}` for a multi-part source. Stable across re-derivation so links + embeddings migrate.
fn derived_doc_id(media_id: &str, extractor_id: &str, part: &ExtractedDoc) -> String {
    match &part.part {
        Some(p) => format!("derived_from-{media_id}:{extractor_id}:{p}"),
        None => format!("derived_from-{media_id}:{extractor_id}"),
    }
}

fn non_empty(s: &str) -> Option<String> {
    let t = s.trim();
    (!t.is_empty()).then(|| t.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lb_extract::Extractor;

    /// A test-only extractor that PANICS — a `#[cfg(test)]` fixture (allowed; not a fake backend).
    /// It proves `run_contained` turns a parser panic into a `Failed` outcome, so a panicking
    /// extractor is contained to its item and never unwinds out of the batch loop (scope requirement
    /// "panicking extractor contained to its item"). We can't rely on a real parser panicking on cue.
    struct PanickingExtractor;
    impl Extractor for PanickingExtractor {
        fn id(&self) -> &'static str {
            "panic-test"
        }
        fn version(&self) -> u32 {
            1
        }
        fn extract(
            &self,
            _bytes: &[u8],
            _mime: &str,
            _opts: &ExtractOpts,
        ) -> Result<Vec<ExtractedDoc>, ExtractError> {
            panic!("boom — untrusted input tripped the parser");
        }
    }

    #[test]
    fn panic_is_contained_as_failed() {
        let ex = PanickingExtractor;
        let out = run_contained(
            &ex,
            b"anything",
            "application/x-test",
            &ExtractOpts::default(),
        );
        assert!(matches!(out, Err(ExtractError::Failed(_))), "got {out:?}");
    }
}
