//! Snapshot tests — the **fidelity contract** for every v1 extractor (doc-extraction scope: "the
//! fidelity contract; snapshot fixtures make regressions visible"). Each committed `snapshots/*.snap`
//! is the exact markdown a real fixture binary must extract to; a diff is a loud, reviewable change.
//!
//! Determinism is the whole point: these run offline (no network anywhere — pure parsers) against
//! committed fixture binaries, so the same bytes always produce the same snapshot. Regenerate after
//! an intentional change with `UPDATE_SNAPSHOTS=1 cargo test -p lb-extract --test snapshot`.

use lb_extract::{extractor_for, ExtractOpts, ExtractedDoc, SplitPolicy, MIME_XLSX};

const XLSX: &str = MIME_XLSX;

/// Extract `path` as `mime` and render the docs to a stable text form for snapshotting.
fn render(mime: &str, path: &str, opts: ExtractOpts) -> String {
    let bytes = std::fs::read(fixture(path)).expect("fixture readable");
    let ex = extractor_for(mime).expect("extractor registered for mime");
    let docs = ex.extract(&bytes, mime, &opts).expect("extract ok");
    render_docs(ex.id(), ex.version(), &docs)
}

fn render_docs(id: &str, version: u32, docs: &[ExtractedDoc]) -> String {
    let mut out = format!("extractor: {id} v{version}\ndocs: {}\n", docs.len());
    for (i, d) in docs.iter().enumerate() {
        out.push_str(&format!(
            "\n--- doc {i} part={:?} title={:?} ---\n{}\n",
            d.part, d.title_hint, d.markdown
        ));
    }
    out
}

fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

/// Compare `actual` against the committed snapshot `name`, or write it under `UPDATE_SNAPSHOTS=1`.
fn assert_snapshot(name: &str, actual: &str) {
    let path = format!("{}/tests/snapshots/{name}.snap", env!("CARGO_MANIFEST_DIR"));
    if std::env::var("UPDATE_SNAPSHOTS").is_ok() {
        std::fs::write(&path, actual).expect("write snapshot");
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("missing snapshot {path}; run with UPDATE_SNAPSHOTS=1"));
    assert_eq!(
        actual, expected,
        "snapshot {name} drifted — review the diff; regenerate with UPDATE_SNAPSHOTS=1 if intended"
    );
}

#[test]
fn pdf_multipage_text_layer() {
    assert_snapshot(
        "pdf_report",
        &render("application/pdf", "report.pdf", ExtractOpts::default()),
    );
}

#[test]
fn xlsx_whole_workbook_default() {
    assert_snapshot(
        "xlsx_whole",
        &render(XLSX, "workbook.xlsx", ExtractOpts::default()),
    );
}

#[test]
fn xlsx_per_sheet_split() {
    let opts = ExtractOpts {
        split: SplitPolicy::PerPart,
        ..Default::default()
    };
    assert_snapshot("xlsx_per_part", &render(XLSX, "workbook.xlsx", opts));
}

#[test]
fn csv_table() {
    assert_snapshot(
        "csv_table",
        &render("text/csv", "table.csv", ExtractOpts::default()),
    );
}

#[test]
fn html_to_markdown() {
    assert_snapshot(
        "html_page",
        &render("text/html", "page.html", ExtractOpts::default()),
    );
}

#[test]
fn markdown_passthrough() {
    assert_snapshot(
        "text_notes",
        &render("text/markdown", "notes.md", ExtractOpts::default()),
    );
}
