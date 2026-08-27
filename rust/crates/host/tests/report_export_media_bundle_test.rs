//! What the JSON bridge does with a snapshot bundle it cannot use: refuse LOUDLY, naming the cell
//! and the actual mistake.
//!
//! The capture side already skips a block it could not rasterise — an honest gap the node places as
//! a titled tile. So a payload that ARRIVED and did not decode is a wire bug, and swallowing it
//! would turn a fixable client defect into a PDF that looks complete and is not. Sibling of
//! `report_export_media_test.rs`; the shared fixture is `support/report_media.rs`.

use lb_host::{report_export_media, ExportOptions, ReportError};
use lb_store::Store;
use serde_json::json;

#[path = "support/report_media.rs"]
mod support;
use support::{principal, report_cell, save_report_dashboard, upload_json, ALL};

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
    let err = report_export_media(
        &store,
        &p,
        ws,
        "energy",
        Some(&id),
        &ExportOptions::default(),
        1,
    )
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
    let err = report_export_media(
        &store,
        &p,
        ws,
        "energy",
        Some(&not_a_bundle),
        &ExportOptions::default(),
        1,
    )
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
        report_export_media(
            &store,
            &p,
            ws,
            "energy",
            Some(&empty),
            &ExportOptions::default(),
            1
        )
        .await
        .is_ok(),
        "an explicitly empty bundle composes the skeleton"
    );
}
