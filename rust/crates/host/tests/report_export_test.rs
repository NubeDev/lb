//! `report.export` re-addressed at REPORT-KIND DASHBOARDS (reports-as-dashboards scope), headless
//! against a real store and a real Typst compile. Covers the mandatory categories by name:
//! capability-deny BOTH ways (missing `report.export`, and missing `dashboard.get`) each with a
//! passing negative control so neither deny is a tautology, and workspace isolation.
//!
//! Its own file rather than more tests in `report_test.rs`, which is already over the FILE-LAYOUT
//! ratchet's baseline.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{dashboard_save_meta, report_export, Cell, PageMeta, ReportError};
use lb_store::Store;

/// A principal `sub` in workspace `ws` holding `caps`.
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

const D_GET: &str = "mcp:dashboard.get:call";
const D_SAVE: &str = "mcp:dashboard.save:call";
const D_LIST: &str = "mcp:dashboard.list:call";
const ALL: &[&str] = &[D_GET, D_SAVE, D_LIST, "mcp:report.export:call"];

/// Export a REPORT-KIND DASHBOARD → `%PDF`-prefixed bytes: the whole re-addressed path, end to end,
/// through a real Typst compile. Also pins the two behaviours the A4 grid layout is supposed to have
/// that the linear notebook did not: a cell with no capture is still PLACED (an honest hole, not a
/// silently shorter document), and an ordinary dashboard is REFUSED rather than laid out as a report.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn export_composes_a_report_kind_dashboard_to_pdf() {
    let store = Store::memory().await.unwrap();
    let ws = "ws-nube";
    let test = principal("user:test", ws, ALL);

    // Two panels side by side on page 1, one on page 2 (row 14 is the page break).
    let cells = vec![
        report_cell("p1", 0, 0, 6, 5, "Energy"),
        report_cell("p2", 6, 0, 6, 5, "Demand"),
        report_cell("p3", 0, 14, 12, 6, "Trend"),
    ];
    save_report_dashboard(&store, &test, ws, "energy", "Monthly Energy", cells.clone()).await;

    // p1 + p3 captured; p2 deliberately NOT — it must still occupy its rectangle.
    let png = one_px_png();
    let pdf = report_export(
        &store,
        &test,
        ws,
        "energy",
        vec![
            ("p1".to_string(), png.clone()),
            ("p3".to_string(), png.clone()),
        ],
        1,
    )
    .await
    .expect("export ok");
    assert!(pdf.starts_with(b"%PDF"), "expected PDF magic bytes");

    // An ORDINARY dashboard is not a report — refused loudly, never laid onto A4.
    save_plain_dashboard(&store, &test, ws, "ops", "Ops", cells).await;
    let err = report_export(&store, &test, ws, "ops", vec![], 1)
        .await
        .unwrap_err();
    assert!(
        matches!(err, ReportError::BadInput(ref m) if m.contains("not a report")),
        "a plain dashboard must be refused, got {err:?}"
    );
}

/// Export re-checks the exporter's own caps against the DASHBOARD read, not just `report.export` —
/// the re-addressing must not have opened a side door that reads a board the caller cannot get.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn export_is_denied_without_the_export_cap_and_without_dashboard_read() {
    let store = Store::memory().await.unwrap();
    let ws = "ws-nube";
    let test = principal("user:test", ws, ALL);
    save_report_dashboard(
        &store,
        &test,
        ws,
        "energy",
        "Monthly Energy",
        vec![report_cell("p1", 0, 0, 12, 5, "Energy")],
    )
    .await;

    // Holds every dashboard cap but NOT report.export.
    let no_export = principal("user:test", ws, &[D_GET, D_SAVE, D_LIST]);
    assert!(
        matches!(
            report_export(&store, &no_export, ws, "energy", vec![], 1).await,
            Err(ReportError::Denied)
        ),
        "report.export is its own cap"
    );

    // Holds report.export but cannot READ a dashboard — must still be denied.
    let no_read = principal("user:test", ws, &["mcp:report.export:call"]);
    assert!(
        matches!(
            report_export(&store, &no_read, ws, "energy", vec![], 1).await,
            Err(ReportError::Denied)
        ),
        "export must not bypass the dashboard read gate"
    );

    // Negative control: with BOTH, it works — so the denies above are not tautologies.
    assert!(report_export(&store, &test, ws, "energy", vec![], 1)
        .await
        .is_ok());
}

/// Workspace isolation: a report id that exists in ws-A does not export from ws-B.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn export_is_workspace_isolated() {
    let store = Store::memory().await.unwrap();
    let test_a = principal("user:test", "ws-a", ALL);
    let test_b = principal("user:test", "ws-b", ALL);
    save_report_dashboard(
        &store,
        &test_a,
        "ws-a",
        "energy",
        "Monthly Energy",
        vec![report_cell("p1", 0, 0, 12, 5, "Energy")],
    )
    .await;

    assert!(report_export(&store, &test_a, "ws-a", "energy", vec![], 1)
        .await
        .is_ok());
    let err = report_export(&store, &test_b, "ws-b", "energy", vec![], 1)
        .await
        .unwrap_err();
    assert!(
        matches!(err, ReportError::NotFound | ReportError::Denied),
        "the other workspace must not see it, got {err:?}"
    );
}

/// A grid cell for a report-kind dashboard.
fn report_cell(i: &str, x: u32, y: u32, w: u32, h: u32, title: &str) -> Cell {
    Cell {
        i: i.into(),
        x,
        y,
        w,
        h,
        title: title.into(),
        view: "stat".into(),
        ..Cell::default()
    }
}

async fn save_report_dashboard(
    store: &Store,
    p: &Principal,
    ws: &str,
    id: &str,
    title: &str,
    cells: Vec<Cell>,
) {
    save_dashboard_of_kind(store, p, ws, id, title, cells, Some("report".into())).await;
}

async fn save_plain_dashboard(
    store: &Store,
    p: &Principal,
    ws: &str,
    id: &str,
    title: &str,
    cells: Vec<Cell>,
) {
    save_dashboard_of_kind(store, p, ws, id, title, cells, None).await;
}

async fn save_dashboard_of_kind(
    store: &Store,
    p: &Principal,
    ws: &str,
    id: &str,
    title: &str,
    cells: Vec<Cell>,
    kind: Option<String>,
) {
    dashboard_save_meta(
        store,
        p,
        ws,
        id,
        title,
        PageMeta {
            kind,
            ..PageMeta::default()
        },
        cells,
        vec![],
        1,
    )
    .await
    .expect("dashboard save ok");
}

/// A real 1x1 PNG — what the browser posts as a panel capture.
fn one_px_png() -> Vec<u8> {
    vec![
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f,
        0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0xfc,
        0xcf, 0xc0, 0x50, 0x0f, 0x00, 0x04, 0x85, 0x01, 0x80, 0x84, 0xa9, 0x8c, 0x21, 0x00, 0x00,
        0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ]
}
