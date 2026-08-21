//! `report.export` **over the bridge** — `POST /mcp/call`, a real node, a real store, a real Typst
//! compile. No mocks (CLAUDE §9 / testing §0).
//!
//! **This file is the whole point of Track A, and it exists because the direct-call tests cannot
//! see what it sees.** `tool_gate.rs` records four verbs that shipped, passed review, appeared in
//! the catalog, and answered a bare `denied` to every caller including admins — `media.upload_*`,
//! `series.retention.delete`, `series.rollup.read`, `outbox.enqueue_held`. Every one of them was
//! green in tests that called the host function directly, because a direct call never crosses the
//! OUTER gate where `gate_tool_for` decides which capability is actually demanded. Each was found
//! by a human driving it on a live node.
//!
//! So the two tests that matter here are:
//!
//!   • **Reachability** — a principal holding the REAL role-bundle caps (minted through
//!     `provision_member`, i.e. whatever the durable grant chain actually resolves to, not a
//!     hand-written caps list) can drive the whole three-step export over the bridge and get
//!     `%PDF-` bytes back. This is the test that would have caught all four incidents.
//!   • **Deny, over the bridge** — a principal without `mcp:report.export:call` is refused at
//!     `/mcp/call` with no existence signal, with a passing negative control beside it so the deny
//!     is not a tautology.
//!
//! Plus workspace isolation and the loud refusal of an ordinary dashboard, both driven the same way.

mod common;

use axum::http::StatusCode;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use common::bootstrap::provision_member;
use common::{bearer, gateway, json_body, json_post, token, NOW};
use lb_role_gateway::{router, Gateway};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tower::ServiceExt;

/// `POST /mcp/call` as `bearer` with `{tool, args}`; return `(status, json)`.
async fn mcp_call(gw: &Gateway, bearer_tok: &str, tool: &str, args: Value) -> (StatusCode, Value) {
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post("/mcp/call", json!({ "tool": tool, "args": args })),
            bearer_tok,
        ))
        .await
        .unwrap();
    let status = resp.status();
    let body = if status == StatusCode::OK {
        json_body::<Value>(resp).await
    } else {
        Value::Null
    };
    (status, body)
}

/// The caps a hand-minted token needs for the full round trip. Used ONLY by the deny tests, which
/// need to remove one cap at a time; the reachability test deliberately uses the real role bundle
/// instead, because a hand-written list cannot tell you whether anything grants these in practice.
const EXPORT: &str = "mcp:report.export:call";
const ALL: &[&str] = &[
    "mcp:dashboard.get:call",
    "mcp:dashboard.save:call",
    EXPORT,
    // ⚠ The GATE, not the tool — `gate_tool_for` aliases all three upload phases onto this one.
    "mcp:media.upload:call",
    "mcp:media.read:call",
    "store:media/**:read",
];

/// **THE REACHABILITY TEST.** A real member, with whatever caps the real grant chain resolves to,
/// drives the whole shape over the bridge: snapshots up through `media.upload_*`, one
/// `report.export` trading ids, PDF down through `media.read`.
///
/// If `report.export` ever gains a `gate_tool_for` alias onto a cap no bundle grants, or if the
/// member bundle loses `mcp:report.export:call`, this test goes red. Nothing else in the tree
/// would.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_real_member_exports_a_report_over_the_bridge() {
    let (gw, _key) = gateway().await;
    let ws = "nube";
    let tok = provision_member(&gw, "user:test", ws).await;

    seed_report_board(&gw, &tok, "energy", "Monthly Energy").await;

    // 1 — the snapshot bundle up, entirely over the bridge. Three verbs, no HTTP byte route, no
    // Authorization header anywhere: this is exactly what an extension page can do.
    let bundle = json!({ "snapshots": [
        { "cellId": "p1", "png": BASE64.encode(one_px_png()) },
    ]});
    let snapshot_id = upload_over_bridge(&gw, &tok, &bundle).await;

    // 2 — compose.
    let (status, reply) = mcp_call(
        &gw,
        &tok,
        "report.export",
        json!({ "id": "energy", "snapshotMediaId": snapshot_id, "now": NOW }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a real member must be able to export — got {status} (a bare denial here is the \
         shipped-but-unusable failure `tool_gate.rs` documents four times)"
    );
    let pdf_id = reply["pdfMediaId"]
        .as_str()
        .expect("the reply carries a pdf media id")
        .to_string();
    assert_eq!(reply["mime"], "application/pdf");

    // 3 — the bytes down, over the bridge, slice by slice until eof.
    let pdf = read_over_bridge(&gw, &tok, &pdf_id).await;
    assert!(
        pdf.starts_with(b"%PDF"),
        "real bytes out of the real Typst engine"
    );
    assert_eq!(
        pdf.len() as u64,
        reply["bytes"].as_u64().unwrap(),
        "`bytes` must be the real total the caller walks to"
    );
}

/// **THE MANDATORY DENY, DRIVEN OVER THE BRIDGE.** A principal without `mcp:report.export:call` is
/// refused at `/mcp/call` with no existence signal — and the negative control beside it proves the
/// refusal is about the cap and not about the request being broken.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_export_is_denied_over_the_bridge_without_its_own_cap() {
    let (gw, key) = gateway().await;
    let ws = "nube";
    let granted = token(&key, "user:test", ws, ALL);
    seed_report_board(&gw, &granted, "energy", "Monthly Energy").await;

    // Every cap EXCEPT the export one. `report.export` is a concrete cap, deliberately not covered
    // by any `mcp:*.*:call` wildcard — view-without-export is a real posture.
    let without: Vec<&str> = ALL.iter().copied().filter(|c| *c != EXPORT).collect();
    let denied_tok = token(&key, "user:test", ws, &without);

    let (status, _) = mcp_call(&gw, &denied_tok, "report.export", json!({ "id": "energy" })).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "no export cap must be an opaque 403 at the bridge"
    );

    // Negative control — the SAME call with the cap succeeds, so the deny above is about the
    // capability rather than about a malformed request.
    let (status, _) = mcp_call(&gw, &granted, "report.export", json!({ "id": "energy" })).await;
    assert_eq!(status, StatusCode::OK, "the granted call must work");
}

/// The export must not become a side door onto a board the caller cannot read: holding
/// `report.export` while lacking `dashboard.get` is still a refusal.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_export_does_not_bypass_the_dashboard_read_gate() {
    let (gw, key) = gateway().await;
    let ws = "nube";
    let granted = token(&key, "user:test", ws, ALL);
    seed_report_board(&gw, &granted, "energy", "Monthly Energy").await;

    let without_read: Vec<&str> = ALL
        .iter()
        .copied()
        .filter(|c| *c != "mcp:dashboard.get:call")
        .collect();
    let tok = token(&key, "user:test", ws, &without_read);

    let (status, _) = mcp_call(&gw, &tok, "report.export", json!({ "id": "energy" })).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "export re-runs `dashboard_get`'s gates under the same principal"
    );
}

/// Workspace isolation over the bridge: ws-B exporting ws-A's board id fails, and fails the same
/// way a missing id does.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_bridge_export_is_workspace_isolated() {
    let (gw, key) = gateway().await;
    let a = token(&key, "user:test", "ws-a", ALL);
    let b = token(&key, "user:other", "ws-b", ALL);
    seed_report_board(&gw, &a, "energy", "Monthly Energy").await;

    let (status_a, _) = mcp_call(&gw, &a, "report.export", json!({ "id": "energy" })).await;
    assert_eq!(status_a, StatusCode::OK, "the owner can export");

    let (status_b, _) = mcp_call(&gw, &b, "report.export", json!({ "id": "energy" })).await;
    assert_ne!(status_b, StatusCode::OK, "ws-B must not reach ws-A's board");

    // And the refusal must be indistinguishable from a board that simply does not exist — otherwise
    // the status code is an existence oracle for another workspace's records.
    let (status_missing, _) = mcp_call(&gw, &b, "report.export", json!({ "id": "no-such" })).await;
    assert_eq!(
        status_b, status_missing,
        "cross-workspace and missing must answer identically"
    );
}

/// An ordinary dashboard is refused loudly, with the message naming the kind — already true of the
/// route, and it must stay true on the bridge. A 12-column board laid onto a 166 mm page is not a
/// report, it is a broken PDF, and a loud refusal sends the author to "New report".
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_ordinary_dashboard_is_refused_over_the_bridge() {
    let (gw, key) = gateway().await;
    let ws = "nube";
    let tok = token(&key, "user:test", ws, ALL);

    // A plain board — no `kind`.
    let (status, _) = mcp_call(
        &gw,
        &tok,
        "dashboard.save",
        json!({
            "id": "ops", "title": "Ops", "cells": [cell("p1", "Energy")], "now": NOW,
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "seed ok");

    let (status, _) = mcp_call(&gw, &tok, "report.export", json!({ "id": "ops" })).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "a plain dashboard is a BAD REQUEST, not a denial and not a silent PDF"
    );
}

// ── helpers ──────────────────────────────────────────────────────────────────────────────────

/// Author a report-kind dashboard through the bridge — `dashboard.save { kind: "report" }`, which
/// is the ONLY way to author a record `report.export` can see. `report.save` writes the retired
/// `report:{id}` notebook table, a different store with no shim between them.
async fn seed_report_board(gw: &Gateway, tok: &str, id: &str, title: &str) {
    let (status, _) = mcp_call(
        gw,
        tok,
        "dashboard.save",
        json!({
            "id": id,
            "title": title,
            "kind": "report",
            "cells": [cell("p1", "Energy")],
            "now": NOW,
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "seeding the report board");
}

fn cell(i: &str, title: &str) -> Value {
    json!({ "i": i, "x": 0, "y": 0, "w": 12, "h": 5, "view": "stat", "title": title })
}

/// Upload a JSON document entirely over the bridge — begin → chunk_write → commit, the same three
/// verbs the kit's `uploadMedia` drives for site photographs.
async fn upload_over_bridge(gw: &Gateway, tok: &str, doc: &Value) -> String {
    let bytes = serde_json::to_vec(doc).unwrap();
    let checksum: String = {
        let mut h = Sha256::new();
        h.update(&bytes);
        h.finalize().iter().map(|b| format!("{b:02x}")).collect()
    };

    let (status, begun) = mcp_call(
        gw,
        tok,
        "media.upload_begin",
        json!({ "mime": "application/json", "bytes": bytes.len(), "checksum": checksum, "now": NOW }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "media.upload_begin over the bridge");
    let id = begun["id"].as_str().unwrap().to_string();
    let chunk_size = begun["chunk_size"].as_u64().unwrap() as usize;

    for (n, chunk) in bytes.chunks(chunk_size).enumerate() {
        let (status, _) = mcp_call(
            gw,
            tok,
            "media.chunk_write",
            json!({ "id": id, "n": n, "bytes": BASE64.encode(chunk) }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "media.chunk_write over the bridge");
    }

    let (status, _) = mcp_call(
        gw,
        tok,
        "media.upload_commit",
        json!({ "id": id, "now": NOW }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "media.upload_commit over the bridge"
    );
    id
}

/// Walk a media item down over the bridge, slice by slice until `eof` — exactly what the kit's
/// `readMediaBlob` does.
async fn read_over_bridge(gw: &Gateway, tok: &str, id: &str) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let mut offset = 0u64;
    for _ in 0..64 {
        let (status, slice) =
            mcp_call(gw, tok, "media.read", json!({ "id": id, "offset": offset })).await;
        assert_eq!(status, StatusCode::OK, "media.read over the bridge");
        out.extend_from_slice(
            &BASE64
                .decode(slice["bytes"].as_str().unwrap_or_default())
                .expect("valid base64"),
        );
        if slice["eof"].as_bool().unwrap_or(false) {
            return out;
        }
        let len = slice["len"].as_u64().unwrap_or(0);
        assert!(len > 0, "an unmoving cursor would loop forever");
        offset += len;
    }
    panic!("media.read did not terminate within 64 slices");
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
