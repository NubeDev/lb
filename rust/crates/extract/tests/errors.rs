//! The honest-failure contract (doc-extraction scope) — the cases where an extractor must return
//! a specific error rather than a silent empty doc. These are the negative half of the fidelity
//! contract: `unsupported` (we don't do this) vs `failed` (the bytes were bad) must never blur into
//! an empty markdown doc. Offline, pure — no network.

use lb_extract::{extractor_for, ExtractError, ExtractOpts, MIME_XLSX};

fn extract(mime: &str, bytes: &[u8]) -> Result<Vec<lb_extract::ExtractedDoc>, ExtractError> {
    let ex = extractor_for(mime).expect("extractor for mime");
    ex.extract(bytes, mime, &ExtractOpts::default())
}

#[test]
fn unknown_mime_has_no_extractor() {
    // The registry returns None → the host maps this to the per-item `unsupported` outcome.
    assert!(extractor_for("application/zip").is_none());
    assert!(extractor_for("image/png").is_none());
    assert!(!lb_extract::is_supported("application/octet-stream"));
}

#[test]
fn mime_parameters_and_casing_still_resolve() {
    // A caller's exact header (parameters, casing) must not cause a spurious unsupported.
    assert!(lb_extract::is_supported("text/HTML; charset=utf-8"));
    assert!(lb_extract::is_supported("Application/PDF"));
}

#[test]
fn corrupt_pdf_fails_not_panics() {
    let bytes = std::fs::read(fixture("corrupt.pdf")).unwrap();
    let err = extract("application/pdf", &bytes).unwrap_err();
    assert!(matches!(err, ExtractError::Failed(_)), "got {err:?}");
}

#[test]
fn image_only_pdf_is_unsupported_not_empty() {
    // A PDF with a valid structure but NO text layer (a scan) → honest Unsupported, never an empty
    // doc. We synthesize the "no text" case with the minimal valid empty-content PDF the parser
    // reads to an empty string.
    let bytes = minimal_textless_pdf();
    match extract("application/pdf", &bytes) {
        Err(ExtractError::Unsupported(_)) => {}
        // Some parser builds reject the hand-min PDF outright — a Failed is also acceptable here
        // (still not a silent empty doc, which is the property under test).
        Err(ExtractError::Failed(_)) => {}
        other => panic!("expected Unsupported/Failed, got {other:?}"),
    }
}

#[test]
fn non_utf8_text_fails() {
    // Invalid UTF-8 labeled as text must fail, not become a mojibake doc.
    let err = extract("text/plain", &[0xff, 0xfe, 0x00, 0x9f]).unwrap_err();
    assert!(matches!(err, ExtractError::Failed(_)), "got {err:?}");
}

#[test]
fn empty_csv_renders_empty_table_marker() {
    // Empty tabular input is a *successful* extraction of an empty table (provenance is still
    // valuable), not an error — the marker makes the emptiness explicit.
    let docs = extract("text/csv", b"").unwrap();
    assert_eq!(docs.len(), 1);
    assert!(
        docs[0].markdown.contains("empty table"),
        "{}",
        docs[0].markdown
    );
}

#[test]
fn xlsx_table_cell_cap_truncates_with_marker() {
    // A tiny cap forces truncation; the elision marker must name that rows were dropped (a 10k-row
    // sheet must summarize + link, not inline whole — scope risk).
    let bytes = std::fs::read(fixture("workbook.xlsx")).unwrap();
    let ex = extractor_for(MIME_XLSX).unwrap();
    let opts = ExtractOpts {
        max_table_cells: 4, // header (3 cells) + ~1 row, forcing elision on the 4-row Sales sheet
        ..Default::default()
    };
    let docs = ex.extract(&bytes, MIME_XLSX, &opts).unwrap();
    assert!(
        docs[0].markdown.contains("elided"),
        "expected an elision marker, got:\n{}",
        docs[0].markdown
    );
}

fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

/// A minimal structurally-valid PDF whose single page has an empty content stream — a stand-in for
/// a scanned/image-only PDF (no text layer). The extractor must not silently return an empty doc.
fn minimal_textless_pdf() -> Vec<u8> {
    let objs: [&str; 4] = [
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R >>",
        "<< /Length 0 >>\nstream\n\nendstream",
    ];
    let mut out = String::from("%PDF-1.4\n");
    let mut offsets = Vec::new();
    for (i, body) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.push_str(&format!("{} 0 obj\n{}\nendobj\n", i + 1, body));
    }
    let xref = out.len();
    out.push_str(&format!(
        "xref\n0 {}\n0000000000 65535 f \n",
        objs.len() + 1
    ));
    for off in &offsets {
        out.push_str(&format!("{off:010} 00000 n \n"));
    }
    out.push_str(&format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
        objs.len() + 1
    ));
    out.into_bytes()
}
