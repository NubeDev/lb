//! `ExportOptions` end to end (report-pagination-and-export-options scope), headless against a real
//! store and a real Typst compile — no mocks, real PDF bytes asserted.
//!
//! Three things are proven here and each is the reason a specific line exists:
//!
//! 1. **Absent options are the shipped document.** The whole contract is additive, which is a claim
//!    about BYTES, not an intention — so it is asserted as bytes.
//! 2. **`RenderOptions` is actually plumbed.** `page_numbers` and `index` have rendered for months
//!    and nothing set them. A test that only checked "the export succeeds" would have passed
//!    throughout that period, so this one asserts the output CHANGES.
//! 3. **An unknown paper is refused, loudly and early.** A silent fall back to A4 gives a caller who
//!    asked for Letter a PDF that is wrong in a way they cannot see.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    dashboard_save_meta, report_export, Cell, ExportOptions, PageMeta, RenderedPanel, ReportError,
};
use lb_store::Store;

fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

const ALL: &[&str] = &[
    "mcp:dashboard.get:call",
    "mcp:dashboard.save:call",
    "mcp:dashboard.list:call",
    "mcp:report.export:call",
];

/// THE BACKWARD-COMPATIBILITY GUARANTEE. A caller that sends no options and one that spells out the
/// shipped defaults must produce **the same bytes** — otherwise a client that always sends its whole
/// export profile silently gets a different document from one that sends nothing, and the "additive"
/// claim is false.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn absent_options_and_the_spelled_out_defaults_are_byte_identical() {
    let (store, p, ws) = fixture("energy", cells()).await;

    let absent = export(&store, &p, ws, "energy", &ExportOptions::default()).await;
    let spelled = export(
        &store,
        &p,
        ws,
        "energy",
        &ExportOptions {
            paper: "a4".into(),
            orientation: "portrait".into(),
            margin_x_mm: Some(22.0),
            margin_top_mm: Some(24.0),
            margin_bottom_mm: Some(22.0),
            scale: Some(2.0),
            page_numbers: false,
            index: false,
        },
    )
    .await;

    assert_eq!(
        absent.len(),
        spelled.len(),
        "the default page must be the A4 page, byte for byte"
    );
    assert_eq!(absent, spelled);
    // …and it really is A4: 210×297mm in PDF points (72dpi), which is what Typst writes as the media
    // box. This is the assertion that would catch a default silently drifting to Letter.
    assert_media_box(&absent, 595.0, 842.0);
}

/// The two-line fix that turns two finished renderer features on. Without the `assembled.options`
/// line in `report_export` this test fails and every other test in the suite still passes — which is
/// exactly how they stayed dark for months.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn page_numbers_and_the_index_actually_reach_the_renderer() {
    let (store, p, ws) = fixture("energy", cells()).await;

    let plain = export(&store, &p, ws, "energy", &ExportOptions::default()).await;
    let numbered = export(
        &store,
        &p,
        ws,
        "energy",
        &ExportOptions {
            page_numbers: true,
            ..ExportOptions::default()
        },
    )
    .await;
    assert_ne!(
        plain, numbered,
        "page_numbers must change the document — if this passes trivially, nothing is plumbed"
    );

    let indexed = export(
        &store,
        &p,
        ws,
        "energy",
        &ExportOptions {
            index: true,
            ..ExportOptions::default()
        },
    )
    .await;
    assert_ne!(plain, indexed, "the index page must change the document");
    assert!(indexed.len() > plain.len(), "an index ADDS a page");
}

/// A non-A4 paper must actually produce that paper's page box — not an A4 page with different
/// arithmetic inside it.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_named_paper_and_orientation_reach_the_page_box() {
    let (store, p, ws) = fixture("energy", cells()).await;

    let letter = export(
        &store,
        &p,
        ws,
        "energy",
        &ExportOptions {
            paper: "letter".into(),
            ..ExportOptions::default()
        },
    )
    .await;
    // 215.9 × 279.4 mm → 612 × 792 pt.
    assert_media_box(&letter, 612.0, 792.0);

    let landscape = export(
        &store,
        &p,
        ws,
        "energy",
        &ExportOptions {
            orientation: "landscape".into(),
            ..ExportOptions::default()
        },
    )
    .await;
    // A4 on its side — the wide edge is now the width.
    assert_media_box(&landscape, 842.0, 595.0);
}

/// Loud and EARLY: the refusal must land before the record is read, so a caller with a typo does not
/// pay for a dashboard read, a brand resolve and a Typst compile to be told they made one.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_unknown_paper_is_a_named_bad_input_not_a_silent_a4() {
    let (store, p, ws) = fixture("energy", cells()).await;

    let err = report_export(
        &store,
        &p,
        ws,
        "energy",
        vec![],
        &ExportOptions {
            paper: "A4 (ish)".into(),
            ..ExportOptions::default()
        },
        1,
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, ReportError::BadInput(ref m) if m.contains("options.paper")),
        "must name the field, got {err:?}"
    );

    // It refuses a report that does not exist for the SAME reason, which proves the check runs before
    // the read rather than after it.
    let early = report_export(
        &store,
        &p,
        ws,
        "no-such-report",
        vec![],
        &ExportOptions {
            paper: "A4 (ish)".into(),
            ..ExportOptions::default()
        },
        1,
    )
    .await
    .unwrap_err();
    assert!(
        matches!(early, ReportError::BadInput(ref m) if m.contains("options.paper")),
        "the option check must precede the record read, got {early:?}"
    );
}

/// THE AUTHORED PAGE BREAK, through the whole real export. Same board, same captures — the only
/// difference is the marker on one cell, and it must add a page.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_marked_cell_starts_a_new_page_in_a_real_export() {
    // Two short bands that comfortably share one page.
    let plain_cells = vec![
        cell("p1", 0, 0, 12, 4, "Cover", false),
        cell("p2", 0, 4, 12, 4, "Trend", false),
    ];
    let marked_cells = vec![
        cell("p1", 0, 0, 12, 4, "Cover", false),
        cell("p2", 0, 4, 12, 4, "Trend", true),
    ];

    let (store, p, ws) = fixture("plain", plain_cells).await;
    save(&store, &p, ws, "marked", marked_cells).await;

    let plain = export(&store, &p, ws, "plain", &ExportOptions::default()).await;
    let marked = export(&store, &p, ws, "marked", &ExportOptions::default()).await;

    assert_eq!(page_count(&plain), 2, "cover + one content page");
    assert_eq!(
        page_count(&marked),
        3,
        "cover + two content pages — the author's break"
    );
}

// ---- helpers ------------------------------------------------------------------------------------

fn cell(i: &str, x: u32, y: u32, w: u32, h: u32, title: &str, break_before: bool) -> Cell {
    Cell {
        i: i.into(),
        x,
        y,
        w,
        h,
        title: title.into(),
        view: "stat".into(),
        page_break_before: break_before,
        ..Cell::default()
    }
}

fn cells() -> Vec<Cell> {
    vec![
        cell("p1", 0, 0, 6, 5, "Energy", false),
        cell("p2", 6, 0, 6, 5, "Demand", false),
    ]
}

async fn fixture(id: &str, cells: Vec<Cell>) -> (Store, Principal, &'static str) {
    let store = Store::memory().await.unwrap();
    let ws = "ws-nube";
    let p = principal("user:test", ws, ALL);
    save(&store, &p, ws, id, cells).await;
    (store, p, ws)
}

async fn save(store: &Store, p: &Principal, ws: &str, id: &str, cells: Vec<Cell>) {
    dashboard_save_meta(
        store,
        p,
        ws,
        id,
        "Report",
        PageMeta {
            kind: Some("report".into()),
            ..PageMeta::default()
        },
        cells,
        vec![],
        1,
    )
    .await
    .expect("dashboard save ok");
}

async fn export(
    store: &Store,
    p: &Principal,
    ws: &str,
    id: &str,
    options: &ExportOptions,
) -> Vec<u8> {
    let pdf = report_export(
        store,
        p,
        ws,
        id,
        vec![RenderedPanel {
            cell_id: "p1".into(),
            png: one_px_png(),
            ..RenderedPanel::default()
        }],
        options,
        1,
    )
    .await
    .expect("export ok");
    assert!(pdf.starts_with(b"%PDF"), "expected PDF magic bytes");
    pdf
}

/// How many pages the PDF has. `/Type /Page` but never `/Type /Pages` (the tree node).
fn page_count(pdf: &[u8]) -> usize {
    let s = String::from_utf8_lossy(pdf);
    s.match_indices("/Type/Page")
        .filter(|(i, _)| !s[*i..].starts_with("/Type/Pages"))
        .count()
        + s.match_indices("/Type /Page")
            .filter(|(i, _)| !s[*i..].starts_with("/Type /Pages"))
            .count()
}

/// The page box Typst wrote, in PDF points, to the nearest point.
fn assert_media_box(pdf: &[u8], w_pt: f64, h_pt: f64) {
    let s = String::from_utf8_lossy(pdf);
    let at = s.find("/MediaBox").expect("a PDF has a media box");
    let open = s[at..].find('[').expect("media box array") + at;
    let close = s[open..].find(']').expect("media box array end") + open;
    let nums: Vec<f64> = s[open + 1..close]
        .split_whitespace()
        .filter_map(|n| n.parse::<f64>().ok())
        .collect();
    assert_eq!(nums.len(), 4, "media box is four numbers, got {nums:?}");
    assert!(
        (nums[2] - w_pt).abs() < 1.5 && (nums[3] - h_pt).abs() < 1.5,
        "expected a {w_pt}x{h_pt}pt page, got {}x{}pt",
        nums[2],
        nums[3]
    );
}

fn one_px_png() -> Vec<u8> {
    vec![
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f,
        0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0xfc,
        0xcf, 0xc0, 0x50, 0x0f, 0x00, 0x04, 0x85, 0x01, 0x80, 0x84, 0xa9, 0x8c, 0x21, 0x00, 0x00,
        0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ]
}
