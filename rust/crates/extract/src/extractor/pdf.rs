//! The PDF text-layer extractor (doc-extraction scope) — `pdf-extract` (pure Rust) pulls the
//! embedded text layer to markdown-ish plain text. v1 is text-layer ONLY: a scanned/image-only PDF
//! has no text layer, so it yields nothing → an honest `Unsupported` (OCR is a named non-goal
//! behind this same trait), never a silent empty doc.
//!
//! `pdf-extract` is known to panic on some malformed PDFs. The host contains a per-item panic
//! anyway (scope: "a panicking extractor fails that item, not the job"), but we ALSO catch it here
//! and convert to `Failed` so the pure crate honors its own `Result` contract and the failure
//! carries a reason for the ledger — defense in depth, and it keeps the crate testable in isolation.

use std::panic::{catch_unwind, AssertUnwindSafe};

use crate::error::ExtractError;
use crate::extractor::text::title_from;
use crate::model::{ExtractOpts, ExtractedDoc};
use crate::trait_def::Extractor;

/// PDF text-layer → a single markdown doc.
pub struct PdfExtractor;

impl Extractor for PdfExtractor {
    fn id(&self) -> &'static str {
        "pdf-text"
    }

    fn version(&self) -> u32 {
        1
    }

    fn extract(
        &self,
        bytes: &[u8],
        _mime: &str,
        _opts: &ExtractOpts,
    ) -> Result<Vec<ExtractedDoc>, ExtractError> {
        // Contain a parser panic here too (the host also contains it): turn it into `Failed` with a
        // reason rather than unwinding out of the pure crate.
        let owned = bytes.to_vec();
        let text = catch_unwind(AssertUnwindSafe(|| {
            pdf_extract::extract_text_from_mem(&owned)
        }))
        .map_err(|_| ExtractError::failed("pdf parser panicked on this input"))?
        .map_err(|e| ExtractError::failed(format!("cannot read PDF: {e}")))?;

        let normalized = normalize(&text);
        if normalized.trim().is_empty() {
            // No text layer → almost certainly a scanned/image-only PDF. Refuse honestly; OCR is
            // the follow-up extractor behind this same trait.
            return Err(ExtractError::unsupported(
                "PDF has no extractable text layer (image-only/scanned — OCR is not supported in v1)",
            ));
        }
        Ok(vec![ExtractedDoc::whole(
            title_from(&normalized),
            normalized,
        )])
    }
}

/// Normalize `pdf-extract`'s output: strip a leading BOM, collapse the 3+ blank-line runs its
/// per-page joins leave, and trim. Deterministic — the snapshot fixture pins the exact shape.
fn normalize(text: &str) -> String {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut out = String::with_capacity(text.len());
    let mut blanks = 0;
    for line in text.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            blanks += 1;
            if blanks <= 2 {
                out.push('\n');
            }
        } else {
            blanks = 0;
            out.push_str(line);
            out.push('\n');
        }
    }
    out.trim().to_string()
}
