//! `report.export` over the JSON bridge — the **media-id envelope**, headless against a real store,
//! a real media store and a real Typst compile. No mocks (rule 9): the snapshot bundle is genuinely
//! uploaded through `media.upload_begin`/`chunk_write`/`commit`, the PDF genuinely comes back out of
//! `media.read`, and the bytes asserted on are `%PDF`.
//!
//! The category this file owns is the **ROUND TRIP**: two ids in, one id out, and what the stored
//! result carries. Its siblings own the rest of the envelope's story —
//! `report_export_media_parity_test.rs` (bridge vs route, byte for byte),
//! `report_export_media_bundle_test.rs` (a malformed bundle is refused by name) and
//! `report_export_media_caps_test.rs` (the mandatory deny + workspace-isolation categories). The
//! shared real-infra fixture they all use is `support/report_media.rs`.

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use lb_host::{report_export_media, ExportOptions};
use lb_store::Store;
use serde_json::json;

#[path = "support/report_media.rs"]
mod support;
use support::{
    one_px_png, principal, read_media, report_cell, save_report_dashboard, upload_json, ALL,
};

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
    let reply = report_export_media(
        &store,
        &p,
        ws,
        "energy",
        Some(&snapshot_id),
        &ExportOptions::default(),
        1,
    )
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

    let reply = report_export_media(&store, &p, ws, "energy", None, &ExportOptions::default(), 1)
        .await
        .expect("export with no captures ok");
    let pdf = read_media(&store, &p, ws, reply["pdfMediaId"].as_str().unwrap()).await;
    assert!(pdf.starts_with(b"%PDF"));
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

    let reply = report_export_media(&store, &p, ws, "energy", None, &ExportOptions::default(), 1)
        .await
        .unwrap();
    let media = lb_host::media_get(&store, &p, ws, reply["pdfMediaId"].as_str().unwrap())
        .await
        .expect("the record is readable");
    let as_json = serde_json::to_value(media).unwrap();
    assert_eq!(as_json["origin"], lb_host::REPORT_ORIGIN);
    assert_eq!(as_json["mime"], "application/pdf");
}
