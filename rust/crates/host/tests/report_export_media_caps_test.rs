//! The mandatory categories for the `report.export` media envelope: **capability deny** and
//! **workspace isolation**, at the function level, each with a passing negative control so no deny
//! is a tautology.
//!
//! Capability-deny is covered here AND, mandatorily, over the BRIDGE in `lb-role-gateway`'s
//! `report_bridge_test.rs`. Both exist on purpose: `tool_gate.rs` documents four shipped-but-unusable
//! verbs, every one of them invisible to tests that called the host fn directly. The shared
//! real-infra fixture is `support/report_media.rs`.

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use lb_host::{report_export_media, ExportOptions, ReportError};
use lb_store::Store;
use serde_json::json;

#[path = "support/report_media.rs"]
mod support;
use support::{
    one_px_png, principal, report_cell, save_report_dashboard, upload_json, ALL, D_GET, D_SAVE,
    EXPORT, M_READ, M_UPLOAD,
};

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
            report_export_media(
                &store,
                &no_export,
                ws,
                "energy",
                None,
                &ExportOptions::default(),
                1
            )
            .await,
            Err(ReportError::Denied)
        ),
        "report.export is its own concrete cap"
    );

    // Holds `report.export` but cannot STORE the result. The PDF composed fine; there is nowhere
    // honest to put it, so the call fails rather than returning an id that serves nothing.
    let no_upload = principal("user:test", ws, &[D_GET, D_SAVE, EXPORT, M_READ]);
    assert!(
        matches!(
            report_export_media(
                &store,
                &no_upload,
                ws,
                "energy",
                None,
                &ExportOptions::default(),
                1
            )
            .await,
            Err(ReportError::Denied)
        ),
        "the PDF is stored under the CALLER's media grant, not the host's"
    );

    // Negative control — with everything, it works.
    assert!(
        report_export_media(&store, &p, ws, "energy", None, &ExportOptions::default(), 1)
            .await
            .is_ok()
    );
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

    assert!(report_export_media(
        &store,
        &a,
        "ws-a",
        "energy",
        None,
        &ExportOptions::default(),
        1
    )
    .await
    .is_ok());
    let err = report_export_media(
        &store,
        &b,
        "ws-b",
        "energy",
        None,
        &ExportOptions::default(),
        1,
    )
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

    let err = report_export_media(
        &store,
        &a,
        "ws-a",
        "energy",
        Some(&theirs),
        &ExportOptions::default(),
        1,
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, ReportError::NotFound | ReportError::Denied),
        "media is workspace-namespaced too, got {err:?}"
    );
}
