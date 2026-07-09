//! Nav gates reach — a curated nav is the allow-list of REACHABLE pages (read included), enforced
//! server-side over the REAL gateway + SurrealDB (no mocks, CLAUDE §9). This is the second half of the
//! live finding `viewer_reach_test.rs` began: the `viewer` role made a subject read-ONLY, but a viewer
//! given a nav of exactly ONE page could still OPEN (and render) every other read page by URL. The nav
//! was a pure lens — it filtered the menu but never gated reach (access-model scope).
//!
//! This proves the nav now **gates reach**. At login the subject's resolved nav is turned into
//! `reach:<surface>:view` caps (nav-reach scope); the dedicated page-entry route `GET /surface/{s}`
//! re-checks that cap server-side. So:
//!
//! (a) **Curated ⇒ only that page reaches.** bob given a one-page nav (`dashboards`) 200s
//!     `GET /surface/dashboards` but 403s `GET /surface/ingest` / `/rules` / `/flows` / `/datasources`
//!     — the read-reach a one-page nav must withhold, denied at the server (not just hidden).
//! (b) **Fallback ⇒ all reachable.** alice (an admin with NO curated nav) 200s `GET /surface/<any>` —
//!     the catastrophic-regression guard: deriving reach must NOT lock out a default member/admin.
//! (c) **Shared data routes stay open.** bob still 200s `GET /series` (the SAME list a dashboard tile
//!     reads) — the page-reach gate is orthogonal to data-read, so his allowed dashboard's tiles work.
//! (d) **Workspace isolation.** a ws-B token's reach never leaks from / into ws-A.

mod common;

use axum::http::StatusCode;
use common::{bearer, gateway, get_req, json_post};
use lb_role_gateway::{router, Gateway};
use serde_json::json;
use tower::ServiceExt;

/// Log in over the real `/login` route (password-less dev) and return the bearer token. Reach caps are
/// folded into this token at mint time (login.rs), so the token already carries the caller's reach set.
async fn login(gw: &Gateway, user: &str, ws: &str) -> String {
    let resp = router(gw.clone())
        .oneshot(json_post(
            "/login",
            json!({ "user": user, "workspace": ws }),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "login {user}@{ws} ok");
    let reply: serde_json::Value = common::json_body(resp).await;
    reply["token"].as_str().unwrap().to_string()
}

/// `GET /surface/{surface}` under `token` — the page-reach preflight. `200` iff the caller's nav grants
/// `reach:<surface>:view` (or the fallback wildcard), else an opaque `403`.
async fn surface_status(gw: &Gateway, token: &str, surface: &str) -> StatusCode {
    router(gw.clone())
        .oneshot(bearer(get_req(&format!("/surface/{surface}")), token))
        .await
        .unwrap()
        .status()
}

/// POST helper that asserts a `2xx` (setup calls that must succeed).
async fn ok_post(gw: &Gateway, token: &str, uri: &str, body: serde_json::Value, what: &str) {
    let resp = router(gw.clone())
        .oneshot(bearer(json_post(uri, body), token))
        .await
        .unwrap();
    assert!(
        resp.status().is_success(),
        "{what}: expected 2xx, got {} ({})",
        resp.status(),
        common::body_text(resp).await
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_curated_nav_gates_reach_but_fallback_reaches_all_and_data_stays_open() {
    let (gw, _key) = gateway().await;

    // alice bootstraps as workspace-admin (first login into an empty ws); she adds bob as a member.
    let admin = login(&gw, "user:alice", "acme").await;
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post("/admin/members", json!({ "sub": "user:bob" })),
            &admin,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT, "admin adds bob");

    // ── Admin authors a ONE-PAGE nav (`dashboards`) and makes it workspace-readable so bob can pick
    //    it. `nav.save` is admin-only, so alice (not bob) authors it. Default visibility is private →
    //    share it `workspace` so bob may resolve it. ──
    ok_post(
        &gw,
        &admin,
        "/navs",
        json!({
            "id": "curated",
            "title": "Bob onboarding",
            "items": [ { "kind": "surface", "surface": "dashboards", "label": "Home" } ]
        }),
        "admin saves the one-page nav",
    )
    .await;
    ok_post(
        &gw,
        &admin,
        "/navs/curated/share",
        json!({ "visibility": "workspace" }),
        "admin shares the nav workspace-wide",
    )
    .await;

    // bob PICKS the curated nav as his personal active nav (a member-level write, keyed to his sub).
    let bob_member = login(&gw, "user:bob", "acme").await;
    ok_post(
        &gw,
        &bob_member,
        "/nav/pref",
        json!({ "id": "curated" }),
        "bob picks the curated nav",
    )
    .await;

    // bob RE-LOGS IN so his token is minted with the reach set derived from his now-curated nav.
    let bob = login(&gw, "user:bob", "acme").await;

    // ── (a) Curated ⇒ ONLY the one page reaches. The page he was given 200s; every other page 403s. ──
    assert_eq!(
        surface_status(&gw, &bob, "dashboards").await,
        StatusCode::OK,
        "bob reaches the ONE page his curated nav gave him (reach:dashboards:view)"
    );
    for blocked in [
        "ingest",
        "rules",
        "flows",
        "datasources",
        "telemetry",
        "system",
    ] {
        assert_eq!(
            surface_status(&gw, &bob, blocked).await,
            StatusCode::FORBIDDEN,
            "bob must be DENIED the page `{blocked}` not in his curated nav (read-reach, server-side)"
        );
    }

    // ── (c) Shared data routes stay OPEN: the SAME `GET /series` list a dashboard tile reads still
    //        200s for bob, so his allowed page's tiles render — the reach gate is orthogonal to data. ──
    let resp = router(gw.clone())
        .oneshot(bearer(get_req("/series"), &bob))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "the shared /series data route stays open (a tile on bob's allowed page reads it)"
    );

    // ── (b) Fallback ⇒ ALL reachable. alice has NO curated nav (no pick, no ws-default) → she resolves
    //        to Fallback → `reach:*:view` → she reaches every page. The no-lock-out regression. ──
    for surface in [
        "dashboards",
        "ingest",
        "rules",
        "flows",
        "datasources",
        "system",
    ] {
        assert_eq!(
            surface_status(&gw, &admin, surface).await,
            StatusCode::OK,
            "a fallback subject (admin, no curated nav) reaches every page: `{surface}`"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn reach_is_workspace_walled() {
    let (gw, _key) = gateway().await;

    // Two independent workspaces; each first-login bootstraps its own admin. A curated nav in `acme`
    // must never affect a subject in `beta`, and a `beta` token reaches `beta` normally (fallback).
    let acme_admin = login(&gw, "user:alice", "acme").await;
    ok_post(
        &gw,
        &acme_admin,
        "/navs",
        json!({
            "id": "curated",
            "title": "acme only",
            "items": [ { "kind": "surface", "surface": "dashboards", "label": "Home" } ]
        }),
        "acme admin saves a curated nav",
    )
    .await;
    ok_post(
        &gw,
        &acme_admin,
        "/nav/default",
        json!({ "id": "curated" }),
        "acme admin sets it as the acme workspace default",
    )
    .await;

    // A fresh subject in `beta` (different ws) has NO nav there → fallback → reaches every beta page.
    // The acme workspace-default curated nav is structurally invisible across the ws wall (§7).
    let beta = login(&gw, "user:carol", "beta").await;
    for surface in ["dashboards", "ingest", "rules", "flows"] {
        assert_eq!(
            surface_status(&gw, &beta, surface).await,
            StatusCode::OK,
            "a beta subject is unaffected by acme's curated nav — reaches beta page `{surface}`"
        );
    }
}
