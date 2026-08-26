//! `report.export` over the JSON bridge — the **media-id envelope**, headless against a real store,
//! a real media store and a real Typst compile. No mocks (rule 9): the snapshot bundle is genuinely
//! uploaded through `media.upload_begin`/`chunk_write`/`commit`, the PDF genuinely comes back out of
//! `media.read`, and the bytes asserted on are `%PDF`.
//!
//! The category this file owns is **PARITY**: the bridge verb and the gateway route must produce
//! byte-identical PDFs for the same `(id, snapshots)`. Two doors onto one document is the regression
//! that matters — a divergence would mean an operator's downloaded report and their scheduled
//! emailed report are different documents with the same name.
//!
//! Capability-deny is covered here at the FUNCTION level and, mandatorily, over the BRIDGE in
//! `lb-role-gateway`'s `report_bridge_test.rs`. Both exist on purpose: `tool_gate.rs` documents four
//! shipped-but-unusable verbs, every one of them invisible to tests that called the host fn directly.
//!
//! Its own file rather than more tests in `report_export_test.rs`, per FILE-LAYOUT.

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    dashboard_save_meta, media_chunk_put, media_read, media_upload_begin, media_upload_commit,
    report_export, report_export_media, Cell, PageMeta, RenderedPanel, ReportError,
};
use lb_store::Store;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

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
const EXPORT: &str = "mcp:report.export:call";
/// ⚠ The **GATE**, not the tool. `gate_tool_for` aliases all three upload phases onto this one cap,
/// and no `mcp:media.upload_begin:call` exists in any role bundle — requesting the literal phase
/// name is the shipped-but-unusable trap `tool_gate.rs` records four times.
const M_UPLOAD: &str = "mcp:media.upload:call";
const M_READ: &str = "mcp:media.read:call";

/// Everything the round trip needs. `store:media/**:read` is the per-ITEM gate `media_serve` checks
/// behind `mcp:media.read:call` — a grant that reaches no item is a grant that reaches nothing, which
/// `builtin_roles.rs` already has its own test for.
const ALL: &[&str] = &[
    D_GET,
    D_SAVE,
    EXPORT,
    M_UPLOAD,
    M_READ,
    "store:media/**:read",
];

/// The whole shape, end to end: snapshots up through the shipped upload path, one compose call
/// trading ids, PDF down through the shipped `media.read`.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_bridge_verb_round_trips_snapshots_and_pdf_through_media() {
    let store = Store::memory().await.unwrap();
    let ws = "ws-nube";
    let p = principal("user:test", ws, ALL);

    let cells = vec![
        report_cell("p1", 0, 0, 6, 5, "Energy"),
        report_cell("p2", 6, 0, 6, 5, "Demand"),
    ];
    save_report_dashboard(&store, &p, ws, "energy", "Monthly Energy", cells).await;

    // 1 — the snapshot bundle goes up as an ordinary media record. This is the SAME path the kit
    // already uses for site photographs; nothing here is export-specific.
    let bundle = json!({ "snapshots": [
        { "cellId": "p1", "png": BASE64.encode(one_px_png()) },
        { "cellId": "p2", "png": BASE64.encode(one_px_png()) },
    ]});
    let snapshot_id = upload_json(&store, &p, ws, &bundle).await;

    // 2 — compose. Two ids in, one id out; the reply fits inside `/mcp/call`'s 2 MiB cap with room
    // to spare, which is the whole reason the verb trades ids rather than bytes.
    let reply = report_export_media(&store, &p, ws, "energy", Some(&snapshot_id), 1)
        .await
        .expect("export ok");
    let pdf_id = reply["pdfMediaId"].as_str().expect("a pdf media id");
    assert_eq!(reply["mime"], "application/pdf");
    let declared = reply["bytes"].as_u64().expect("a byte total");
    assert!(declared > 0, "an empty PDF is not a PDF");

    // 3 — the bytes walk down through `media.read`, the verb lb added for exactly this caller.
    let pdf = read_media(&store, &p, ws, pdf_id).await;
    assert_eq!(pdf.len() as u64, declared, "`bytes` must be the real total");
    assert!(
        pdf.starts_with(b"%PDF"),
        "the real Typst engine's real output, not a stub"
    );
}

/// **PARITY** — the regression that matters: two doors, one document.
///
/// The gateway route and the bridge verb must compose byte-identical PDFs from the same board and
/// the same captures. If they ever diverge, an operator's downloaded report and the scheduled
/// emailed one are different documents wearing the same name, and nothing would say so.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_bridge_and_the_route_compose_byte_identical_pdfs() {
    let store = Store::memory().await.unwrap();
    let ws = "ws-nube";
    let p = principal("user:test", ws, ALL);
    save_report_dashboard(
        &store,
        &p,
        ws,
        "energy",
        "Monthly Energy",
        vec![report_cell("p1", 0, 0, 12, 5, "Energy")],
    )
    .await;

    let png = one_px_png();

    // The ROUTE's path: snapshots decoded straight out of the request body.
    let via_route = report_export(
        &store,
        &p,
        ws,
        "energy",
        vec![RenderedPanel {
            cell_id: "p1".into(),
            png: png.clone(),
            ..RenderedPanel::default()
        }],
        1,
    )
    .await
    .expect("route export ok");

    // The BRIDGE's path: the same snapshots, carried through the media store instead.
    let bundle = json!({ "snapshots": [{ "cellId": "p1", "png": BASE64.encode(&png) }] });
    let snapshot_id = upload_json(&store, &p, ws, &bundle).await;
    let reply = report_export_media(&store, &p, ws, "energy", Some(&snapshot_id), 1)
        .await
        .expect("bridge export ok");
    let via_bridge = read_media(&store, &p, ws, reply["pdfMediaId"].as_str().unwrap()).await;

    assert_eq!(
        via_route, via_bridge,
        "the two doors must produce the SAME document, byte for byte"
    );
}

/// The snapshot bundle is optional, and omitting it composes the report's skeleton — every cell
/// gets its titled error tile and the page count is unchanged. Never a silent success.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn composing_with_no_snapshots_still_produces_a_pdf() {
    let store = Store::memory().await.unwrap();
    let ws = "ws-nube";
    let p = principal("user:test", ws, ALL);
    save_report_dashboard(
        &store,
        &p,
        ws,
        "energy",
        "Monthly Energy",
        vec![report_cell("p1", 0, 0, 12, 5, "Energy")],
    )
    .await;

    let reply = report_export_media(&store, &p, ws, "energy", None, 1)
        .await
        .expect("export with no captures ok");
    let pdf = read_media(&store, &p, ws, reply["pdfMediaId"].as_str().unwrap()).await;
    assert!(pdf.starts_with(b"%PDF"));
}

/// A malformed bundle is refused LOUDLY, naming what was wrong.
///
/// The capture side already skips a block it could not rasterise — an honest gap the node places as
/// a titled tile. So a payload that ARRIVED and did not decode is a wire bug, and swallowing it
/// would turn a fixable client defect into a PDF that looks complete and is not. The `data:` prefix
/// is called out by name because it is the one mistake the wire contract actually invites.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_snapshot_that_is_not_raw_base64_is_refused_by_name() {
    let store = Store::memory().await.unwrap();
    let ws = "ws-nube";
    let p = principal("user:test", ws, ALL);
    save_report_dashboard(
        &store,
        &p,
        ws,
        "energy",
        "Monthly Energy",
        vec![report_cell("p1", 0, 0, 12, 5, "Energy")],
    )
    .await;

    let bad = json!({ "snapshots": [{ "cellId": "p1", "png": "data:image/png;base64,####" }] });
    let id = upload_json(&store, &p, ws, &bad).await;
    let err = report_export_media(&store, &p, ws, "energy", Some(&id), 1)
        .await
        .unwrap_err();
    assert!(
        matches!(err, ReportError::BadInput(ref m) if m.contains("p1") && m.contains("data:")),
        "the refusal must name the cell and the actual mistake, got {err:?}"
    );

    // A document that does not carry `snapshots` at all is refused too — it is a wire bug, and the
    // caller who genuinely wants no captures omits `snapshotMediaId` instead. Defaulting the field
    // to empty would answer a half-written blob with a plausible PDF of error tiles.
    let not_a_bundle = upload_json(&store, &p, ws, &json!({ "hello": "world" })).await;
    let err = report_export_media(&store, &p, ws, "energy", Some(&not_a_bundle), 1)
        .await
        .unwrap_err();
    assert!(
        matches!(err, ReportError::BadInput(ref m) if m.contains("snapshotMediaId")),
        "the refusal must point at the honest way to ask for a skeleton, got {err:?}"
    );

    // An EXPLICITLY empty bundle is a different statement and is honoured: the caller captured
    // nothing and says so.
    let empty = upload_json(&store, &p, ws, &json!({ "snapshots": [] })).await;
    assert!(
        report_export_media(&store, &p, ws, "energy", Some(&empty), 1)
            .await
            .is_ok(),
        "an explicitly empty bundle composes the skeleton"
    );
}

/// Capability-deny at the function level, each with a passing negative control so no deny is a
/// tautology. The mandatory BRIDGE-driven version lives in `report_bridge_test.rs`.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_bridge_verb_denies_without_export_and_without_media_upload() {
    let store = Store::memory().await.unwrap();
    let ws = "ws-nube";
    let p = principal("user:test", ws, ALL);
    save_report_dashboard(
        &store,
        &p,
        ws,
        "energy",
        "Monthly Energy",
        vec![report_cell("p1", 0, 0, 12, 5, "Energy")],
    )
    .await;

    // No `report.export` — refused before a single media byte is read.
    let no_export = principal("user:test", ws, &[D_GET, D_SAVE, M_UPLOAD, M_READ]);
    assert!(
        matches!(
            report_export_media(&store, &no_export, ws, "energy", None, 1).await,
            Err(ReportError::Denied)
        ),
        "report.export is its own concrete cap"
    );

    // Holds `report.export` but cannot STORE the result. The PDF composed fine; there is nowhere
    // honest to put it, so the call fails rather than returning an id that serves nothing.
    let no_upload = principal("user:test", ws, &[D_GET, D_SAVE, EXPORT, M_READ]);
    assert!(
        matches!(
            report_export_media(&store, &no_upload, ws, "energy", None, 1).await,
            Err(ReportError::Denied)
        ),
        "the PDF is stored under the CALLER's media grant, not the host's"
    );

    // Negative control — with everything, it works.
    assert!(report_export_media(&store, &p, ws, "energy", None, 1)
        .await
        .is_ok());
}

/// Workspace isolation: a board id that exists in ws-A does not export from ws-B, and the refusal is
/// indistinguishable from a missing id.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_bridge_verb_is_workspace_isolated() {
    let store = Store::memory().await.unwrap();
    let a = principal("user:test", "ws-a", ALL);
    let b = principal("user:test", "ws-b", ALL);
    save_report_dashboard(
        &store,
        &a,
        "ws-a",
        "energy",
        "Monthly Energy",
        vec![report_cell("p1", 0, 0, 12, 5, "Energy")],
    )
    .await;

    assert!(report_export_media(&store, &a, "ws-a", "energy", None, 1)
        .await
        .is_ok());
    let err = report_export_media(&store, &b, "ws-b", "energy", None, 1)
        .await
        .unwrap_err();
    assert!(
        matches!(err, ReportError::NotFound | ReportError::Denied),
        "the other workspace must not see it, got {err:?}"
    );
}

/// A snapshot bundle from ANOTHER workspace is not readable, so it cannot be composed from.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_snapshot_bundle_from_another_workspace_is_not_reachable() {
    let store = Store::memory().await.unwrap();
    let a = principal("user:test", "ws-a", ALL);
    let b = principal("user:test", "ws-b", ALL);

    // ws-b uploads a bundle; ws-a owns the report.
    let bundle = json!({ "snapshots": [{ "cellId": "p1", "png": BASE64.encode(one_px_png()) }] });
    let theirs = upload_json(&store, &b, "ws-b", &bundle).await;

    save_report_dashboard(
        &store,
        &a,
        "ws-a",
        "energy",
        "Monthly Energy",
        vec![report_cell("p1", 0, 0, 12, 5, "Energy")],
    )
    .await;

    let err = report_export_media(&store, &a, "ws-a", "energy", Some(&theirs), 1)
        .await
        .unwrap_err();
    assert!(
        matches!(err, ReportError::NotFound | ReportError::Denied),
        "media is workspace-namespaced too, got {err:?}"
    );
}

/// The stored PDF carries the `report.export` origin tag.
///
/// Nothing sweeps on it yet — there is no reaping seam in the media store, which is recorded as
/// upstream housekeeping. Tagging it now is what makes that sweep a query rather than a migration,
/// and this test is what stops the tag being dropped in the meantime.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_stored_pdf_is_tagged_as_report_output() {
    let store = Store::memory().await.unwrap();
    let ws = "ws-nube";
    let mut caps: Vec<&str> = ALL.to_vec();
    caps.push("mcp:media.get:call");
    let p = principal("user:test", ws, &caps);
    save_report_dashboard(
        &store,
        &p,
        ws,
        "energy",
        "Monthly Energy",
        vec![report_cell("p1", 0, 0, 12, 5, "Energy")],
    )
    .await;

    let reply = report_export_media(&store, &p, ws, "energy", None, 1)
        .await
        .unwrap();
    let media = lb_host::media_get(&store, &p, ws, reply["pdfMediaId"].as_str().unwrap())
        .await
        .expect("the record is readable");
    let as_json = serde_json::to_value(media).unwrap();
    assert_eq!(as_json["origin"], lb_host::REPORT_ORIGIN);
    assert_eq!(as_json["mime"], "application/pdf");
}

// ── helpers ──────────────────────────────────────────────────────────────────────────────────

/// Upload a JSON document through the REAL three-phase media path and return its id.
async fn upload_json(store: &Store, p: &Principal, ws: &str, doc: &Value) -> String {
    let bytes = serde_json::to_vec(doc).unwrap();
    let checksum = {
        let mut h = Sha256::new();
        h.update(&bytes);
        h.finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    let begun = media_upload_begin(
        store,
        p,
        ws,
        "application/json",
        bytes.len() as u64,
        &checksum,
        Some("test"),
        1,
    )
    .await
    .expect("begin ok");
    let id = begun["id"].as_str().unwrap().to_string();
    for (n, chunk) in bytes.chunks(lb_host::CHUNK_SIZE as usize).enumerate() {
        media_chunk_put(store, p, ws, &id, n as u32, chunk)
            .await
            .expect("chunk ok");
    }
    media_upload_commit(store, p, ws, &id, 1)
        .await
        .expect("commit ok");
    id
}

/// Walk a media item down through `media.read`, exactly as the kit does — slices until `eof`.
async fn read_media(store: &Store, p: &Principal, ws: &str, id: &str) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let mut offset = 0u64;
    for _ in 0..64 {
        let slice = media_read(store, p, ws, id, None, offset, None)
            .await
            .expect("read ok");
        let bytes = BASE64
            .decode(slice["bytes"].as_str().unwrap_or_default())
            .expect("valid base64");
        out.extend_from_slice(&bytes);
        if slice["eof"].as_bool().unwrap_or(false) {
            return out;
        }
        let len = slice["len"].as_u64().unwrap_or(0);
        assert!(len > 0, "an unmoving cursor would loop forever");
        offset += len;
    }
    panic!("media.read did not terminate within 64 slices");
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
    dashboard_save_meta(
        store,
        p,
        ws,
        id,
        title,
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
