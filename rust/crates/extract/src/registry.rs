//! The per-mime extractor registry — the one seam that maps a mime type to its [`Extractor`]
//! (doc-extraction scope). This is the ONLY place that knows the mime→extractor mapping; the host
//! calls [`extractor_for`] and treats a `None` as the per-item `unsupported` outcome. Generic over
//! mime, never a domain noun (rule 10): the registry keys on `application/pdf`, never on what the
//! PDF *is about*.
//!
//! Adding a mime family = adding one extractor file + one arm here. The registry is a pure `match`
//! (not a lazy map) so it is `const`-cheap, allocation-free, and trivially deterministic.

use crate::extractor::{CsvExtractor, HtmlExtractor, PdfExtractor, TextExtractor, XlsxExtractor};
use crate::trait_def::Extractor;

/// The canonical spreadsheet mime (XLSX). `.xls` and ODS are follow-ups behind the same trait.
pub const MIME_XLSX: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";

/// Select the extractor for `mime`, or `None` if unsupported. `mime` may carry parameters
/// (`text/html; charset=utf-8`) — they are stripped and the base type lower-cased before matching,
/// so a caller's exact header casing/parameters never cause a spurious `unsupported`.
pub fn extractor_for(mime: &str) -> Option<Box<dyn Extractor>> {
    let base = mime
        .split(';')
        .next()
        .unwrap_or(mime)
        .trim()
        .to_ascii_lowercase();
    match base.as_str() {
        "application/pdf" => Some(Box::new(PdfExtractor)),
        MIME_XLSX => Some(Box::new(XlsxExtractor)),
        "text/csv" | "application/csv" => Some(Box::new(CsvExtractor)),
        "text/html" | "application/xhtml+xml" => Some(Box::new(HtmlExtractor)),
        // Plain text and markdown pass through unchanged (markdown IS the output format).
        "text/plain" | "text/markdown" | "text/x-markdown" => Some(Box::new(TextExtractor)),
        _ => None,
    }
}

/// Whether any extractor claims `mime`. A cheap pre-check the host can use to fail fast before
/// reading bytes (though the authoritative answer is still `extractor_for(...).is_some()`).
pub fn is_supported(mime: &str) -> bool {
    extractor_for(mime).is_some()
}
