//! **PARITY** — the gateway route and the JSON bridge must compose byte-identical PDFs from the
//! same board, the same captures and the same options.
//!
//! Two doors onto one document is the regression that matters: a divergence would mean an
//! operator's downloaded report and their scheduled emailed report are different documents with the
//! same name, and nothing would say so. Its own file rather than more tests in
//! `report_export_media_test.rs`, per FILE-LAYOUT; the shared fixture is `support/report_media.rs`.

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use lb_host::{report_export, report_export_media, ExportOptions, RenderedPanel};
use lb_store::Store;
use serde_json::json;

#[path = "support/report_media.rs"]
mod support;
use support::{
    one_px_png, principal, read_media, report_cell, save_report_dashboard, upload_json, ALL,
};

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
        &ExportOptions::default(),
        1,
    )
    .await
    .expect("route export ok");

    // The BRIDGE's path: the same snapshots, carried through the media store instead.
    let bundle = json!({ "snapshots": [{ "cellId": "p1", "png": BASE64.encode(&png) }] });
    let snapshot_id = upload_json(&store, &p, ws, &bundle).await;
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
    .expect("bridge export ok");
    let via_bridge = read_media(&store, &p, ws, reply["pdfMediaId"].as_str().unwrap()).await;

    assert_eq!(
        via_route, via_bridge,
        "the two doors must produce the SAME document, byte for byte"
    );

    // …AND THEY MUST CARRY THE OPTIONS IDENTICALLY. Parity on the defaults would still hold if one
    // door quietly ignored `options`, so the same non-default profile goes through both: a different
    // paper, a different orientation and page numbers on — three things that each change the bytes.
    let opts = ExportOptions {
        paper: "letter".into(),
        orientation: "landscape".into(),
        page_numbers: true,
        ..ExportOptions::default()
    };
    let route_opts = report_export(
        &store,
        &p,
        ws,
        "energy",
        vec![RenderedPanel {
            cell_id: "p1".into(),
            png: png.clone(),
            ..RenderedPanel::default()
        }],
        &opts,
        1,
    )
    .await
    .expect("route export ok");
    let reply_opts = report_export_media(&store, &p, ws, "energy", Some(&snapshot_id), &opts, 1)
        .await
        .expect("bridge export ok");
    let bridge_opts = read_media(&store, &p, ws, reply_opts["pdfMediaId"].as_str().unwrap()).await;

    assert_eq!(
        route_opts, bridge_opts,
        "both doors must honour the SAME options, byte for byte"
    );
    assert_ne!(
        route_opts, via_route,
        "…and the options must actually have done something, or this proves nothing"
    );
}
