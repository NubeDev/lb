//! Report + brand + binary-asset routes over the REAL gateway + SurrealDB (no mocks, CLAUDE §9).
//! Proves the `report.*` CRUD round-trips block-for-block, the binary PDF export returns real `%PDF`
//! bytes, and the two mandatory security invariants hold at the server boundary:
//!   - **capability deny** — a token missing `report.export` / `report.save` / `brand.save` is 403'd
//!     server-side (the token's caps, never the body).
//!   - **workspace isolation** — a ws-B token cannot read (or list) a ws-A report (§6: the hard wall).
//!
//! Tokens are minted with exact caps via `common::token`, so each test controls precisely which caps
//! the caller holds — the deny tests withhold one cap and assert the 403.

mod common;

use axum::http::StatusCode;
use common::{bearer, gateway, get_req, token};
use lb_role_gateway::{router, Gateway};
use serde_json::{json, Value};
use tower::ServiceExt;

/// The full report cap bundle (author + read + export). Export addresses a report-kind DASHBOARD and
/// re-gates `dashboard.get` internally, so an exporter needs both `report.export` and `dashboard.get`.
const REPORT_CAPS: &[&str] = &[
    "mcp:dashboard.get:call",
    "mcp:dashboard.save:call",
    "mcp:dashboard.list:call",
    "mcp:report.get:call",
    "mcp:report.list:call",
    "mcp:report.save:call",
    "mcp:report.delete:call",
    "mcp:report.share:call",
    "mcp:report.export:call",
    "mcp:brand.get:call",
    "mcp:brand.list:call",
    "mcp:brand.save:call",
    "store:asset/**:read",
    "store:asset/**:write",
];

async fn post(gw: &Gateway, token: &str, uri: &str, body: Value) -> axum::response::Response {
    router(gw.clone())
        .oneshot(bearer(common::json_post(uri, body), token))
        .await
        .unwrap()
}

async fn get(gw: &Gateway, token: &str, uri: &str) -> axum::response::Response {
    router(gw.clone())
        .oneshot(bearer(get_req(uri), token))
        .await
        .unwrap()
}

/// A report with all three block kinds, saved → read back → blocks intact & ordered.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn save_then_get_round_trips_three_block_kinds_in_order() {
    let (gw, key) = gateway().await;
    let t = token(&key, "user:ada", "acme", REPORT_CAPS);

    let blocks = json!([
        { "kind": "markdown", "body": "# Hello", "pageBreak": true },
        { "kind": "image", "assetId": "logo-1", "caption": "the logo" },
        { "kind": "panel", "cell": { "i": "p1", "x": 0, "y": 0, "w": 6, "h": 4, "view": "stat", "title": "Temp" } },
    ]);
    let resp = post(
        &gw,
        &t,
        "/reports",
        json!({ "id": "r1", "title": "Q1", "blocks": blocks, "brandId": "", "toolbar": {} }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK, "save 200");

    let resp = get(&gw, &t, "/reports/r1").await;
    assert_eq!(resp.status(), StatusCode::OK, "get 200");
    let report: Value = common::json_body(resp).await;
    let got = report["blocks"].as_array().unwrap();
    assert_eq!(got.len(), 3, "three blocks");
    assert_eq!(got[0]["kind"], "markdown");
    assert_eq!(got[0]["body"], "# Hello");
    assert_eq!(got[1]["kind"], "image");
    assert_eq!(got[1]["assetId"], "logo-1");
    assert_eq!(got[2]["kind"], "panel");
    assert_eq!(got[2]["cell"]["i"], "p1");

    // And the roster lists it.
    let resp = get(&gw, &t, "/reports").await;
    assert_eq!(resp.status(), StatusCode::OK);
    let list: Value = common::json_body(resp).await;
    let rows = list["reports"].as_array().unwrap();
    assert!(rows.iter().any(|r| r["id"] == "r1"), "report in roster");
}

/// The binary export returns a real `application/pdf` payload with the `%PDF-` magic bytes — now
/// addressed at a **report-kind dashboard**, saved over the ordinary `POST /dashboards` route with
/// `kind: "report"`. The wire contract is unchanged: same export route, same `{cellId, png}` body.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn export_returns_pdf_bytes() {
    let (gw, key) = gateway().await;
    let t = token(&key, "user:ada", "acme", REPORT_CAPS);

    let cells = json!([
        { "i": "p1", "x": 0, "y": 0, "w": 6, "h": 5, "view": "stat", "title": "Temp" },
        { "i": "p2", "x": 6, "y": 0, "w": 6, "h": 5, "view": "stat", "title": "Load" },
    ]);
    let resp = post(
        &gw,
        &t,
        "/dashboards",
        json!({ "id": "r2", "title": "Export", "kind": "report", "cells": cells, "now": 1 }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK, "dashboard save 200");

    // The kind round-trips over REST (the field the whole re-addressing hangs on).
    let resp = get(&gw, &t, "/dashboards/r2").await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = serde_json::from_slice(
        &axum::body::to_bytes(resp.into_body(), 1024 * 1024)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(body["kind"], "report", "kind survives the REST round-trip");

    // A 1x1 PNG (valid image bytes), base64-encoded, keyed to the cell's `i`. Only p1 is captured —
    // p2 must still produce a page, as an error tile.
    let png_b64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC";
    let resp = post(
        &gw,
        &t,
        "/reports/r2/export.pdf",
        json!({ "snapshots": [{ "cellId": "p1", "png": png_b64 }] }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK, "export 200");
    let ctype = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert_eq!(ctype, "application/pdf", "content-type pdf");
    let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024 * 1024)
        .await
        .unwrap();
    assert!(
        bytes.starts_with(b"%PDF-"),
        "PDF magic bytes, got {:?}",
        &bytes[..bytes.len().min(8)]
    );
}

/// An ORDINARY dashboard is refused at the export route — 400, not a silently broken PDF.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn exporting_a_plain_dashboard_is_refused() {
    let (gw, key) = gateway().await;
    let t = token(&key, "user:ada", "acme", REPORT_CAPS);
    let resp = post(
        &gw,
        &t,
        "/dashboards",
        json!({ "id": "ops", "title": "Ops", "cells": [], "now": 1 }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = post(
        &gw,
        &t,
        "/reports/ops/export.pdf",
        json!({ "snapshots": [] }),
    )
    .await;
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "a plain dashboard is not a report"
    );
}

/// An unknown `kind` is refused at save — 400, never a record that vanishes from both rosters.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_unknown_dashboard_kind_is_refused_at_save() {
    let (gw, key) = gateway().await;
    let t = token(&key, "user:ada", "acme", REPORT_CAPS);
    let resp = post(
        &gw,
        &t,
        "/dashboards",
        json!({ "id": "typo", "title": "Typo", "kind": "reprot", "cells": [], "now": 1 }),
    )
    .await;
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "unknown kind is 400"
    );
}

/// Capability deny — a token missing the specific cap is 403'd server-side (not from the UI).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn missing_caps_are_denied() {
    let (gw, key) = gateway().await;

    // No report.save → POST /reports 403.
    let no_save = token(&key, "user:ada", "acme", &["mcp:report.get:call"]);
    let resp = post(
        &gw,
        &no_save,
        "/reports",
        json!({ "id": "x", "title": "X" }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN, "save denied");

    // No brand.save → POST /brands 403.
    let no_brand = token(&key, "user:ada", "acme", &["mcp:brand.get:call"]);
    let resp = post(&gw, &no_brand, "/brands", json!({ "id": "b", "name": "B" })).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN, "brand save denied");

    // Author a report with a full-cap token, then try to export WITHOUT report.export.
    let full = token(&key, "user:ada", "acme", REPORT_CAPS);
    let resp = post(&gw, &full, "/reports", json!({ "id": "r3", "title": "R3" })).await;
    assert_eq!(resp.status(), StatusCode::OK);
    // No report.export (but holds get) → export 403.
    let no_export = token(&key, "user:ada", "acme", &["mcp:report.get:call"]);
    let resp = post(
        &gw,
        &no_export,
        "/reports/r3/export.pdf",
        json!({ "snapshots": [] }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN, "export denied");
}

/// Workspace isolation — a ws-B token cannot read (or list) a ws-A report (§6, the hard wall).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn workspace_isolation_blocks_cross_ws_read() {
    let (gw, key) = gateway().await;

    // ada authors a report in workspace `acme`.
    let a = token(&key, "user:ada", "acme", REPORT_CAPS);
    let resp = post(
        &gw,
        &a,
        "/reports",
        json!({ "id": "secret", "title": "Acme only" }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK, "ws-A save 200");

    // bob in workspace `globex` (same node, same key) holds the SAME cap grammar but scoped to his
    // own ws — his caps never satisfy the `acme` workspace gate, so the read is opaque `404`.
    let b = token(&key, "user:bob", "globex", REPORT_CAPS);
    let resp = get(&gw, &b, "/reports/secret").await;
    assert_ne!(
        resp.status(),
        StatusCode::OK,
        "ws-B cannot read ws-A report"
    );

    // And the ws-B roster never contains the ws-A report.
    let resp = get(&gw, &b, "/reports").await;
    assert_eq!(resp.status(), StatusCode::OK, "ws-B list ok (own ws)");
    let list: Value = common::json_body(resp).await;
    let rows = list["reports"].as_array().unwrap();
    assert!(
        !rows.iter().any(|r| r["id"] == "secret"),
        "ws-A report must not leak into ws-B roster"
    );
}
