//! The rules routes over the real gateway (rules-workbench scope, Phase 1) — the `rules.*` Playground
//! CRUD + run, end to end. Mirrors the dashboard route tests at the transport boundary: the CRUD
//! round-trip, capability-deny per verb, two-session workspace isolation, the three output kinds
//! (scalar / grid / findings), and the cage/deny honesty cases (a cage error → `400` verbatim; an AI
//! body in a model-less workspace → `400 "AI not configured"`). The gateway re-checks every cap
//! server-side — the workspace + principal come from the token (§7).

mod common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::*;
use lb_auth::SigningKey;
use lb_host::{Node, Role as NodeRole};
use lb_role_gateway::{router, Gateway};
use serde_json::{json, Value};
use tower::ServiceExt; // for `oneshot`

/// The full member cap set for a Playground session: the five rules MCP caps (gated at the bridge) +
/// the `store:rule` surface caps the save/get/list/delete verbs re-check below the bridge (defense in
/// depth — a saved rule is a store record). The grid run additionally needs `store.query` (per-test).
const CAPS: &[&str] = &[
    "mcp:rules.run:call",
    "mcp:rules.save:call",
    "mcp:rules.get:call",
    "mcp:rules.list:call",
    "mcp:rules.delete:call",
    "store:rule:read",
    "store:rule:write",
];

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rules_crud_round_trip_over_the_gateway() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:ada", "acme", CAPS);

    // save (create)
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/rules",
                json!({ "id": "hot", "name": "Hot check", "body": "40 + 2" }),
            ),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let saved: Value = json_body(resp).await;
    assert_eq!(saved["id"], "hot");

    // list returns the roster (200, walled to this workspace) and CONTAINS the saved rule — the real
    // round-trip (the host `rules_list` envelope-unwrap was fixed this slice; see the session doc).
    let resp = router(gw.clone())
        .oneshot(bearer(get_req("/rules"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = json_body(resp).await;
    let ids: Vec<&str> = body["rules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&"hot"), "the roster lists the saved rule");

    // get loads it (the authoritative reopen — round-trips the saved body)
    let resp = router(gw.clone())
        .oneshot(bearer(get_req("/rules/hot"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let r: Value = json_body(resp).await;
    assert_eq!(r["name"], "Hot check");
    assert_eq!(r["body"], "40 + 2");

    // delete → 204, then get is 404; re-delete is a no-op (still 204)
    let resp = router(gw.clone())
        .oneshot(bearer(delete_req("/rules/hot"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    let resp = router(gw.clone())
        .oneshot(bearer(get_req("/rules/hot"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let resp = router(gw.clone())
        .oneshot(bearer(delete_req("/rules/hot"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn run_a_scalar_rule_returns_a_scalar_output() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:ada", "acme", CAPS);

    let resp = router(gw)
        .oneshot(bearer(
            json_post("/rules/run", json!({ "body": "40 + 2" })),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let out: Value = json_body(resp).await;
    assert_eq!(out["output"]["kind"], "scalar");
    assert_eq!(out["output"]["value"], 42);
    assert_eq!(out["ai"]["calls"], 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn run_a_grid_rule_returns_a_grid_output() {
    // A grid rule reads the platform `series` source via `store.query` — so the principal must also hold
    // `mcp:store.query:call`, and the workspace needs seeded series rows (the real ingest path).
    let node = Arc::new(Node::boot_as(NodeRole::Hub).await.expect("node boots"));
    let key = SigningKey::generate();
    let ws = "rules-grid";
    lb_host::seed_iot_demo(&node.store, ws, NOW)
        .await
        .expect("seed");

    let mut caps: Vec<&str> = CAPS.to_vec();
    caps.push("mcp:store.query:call");
    let tok = token(&key, "user:ada", ws, &caps);

    // The last expression is a Grid (a materialized history read over the seeded series) → kind "grid".
    let body = r#"history("series", "cooler.temp", "24h")"#;
    let resp = router(gateway_on(node, &key))
        .oneshot(bearer(
            json_post("/rules/run", json!({ "body": body })),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let out: Value = json_body(resp).await;
    assert_eq!(out["output"]["kind"], "grid");
    assert!(out["output"]["columns"].is_array());
    assert!(!out["output"]["rows"].as_array().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn run_an_alert_rule_returns_findings_with_log_and_budget() {
    let (gw, key) = gateway().await;
    // An `alert` finding is routed to the inbox + outbox after the run, so the principal needs those
    // surface caps in addition to `rules.run` (the shipped host behaviour — an alert raises a real
    // attention item). `emit` would need none; `alert` is the must-deliver path.
    let mut caps: Vec<&str> = CAPS.to_vec();
    caps.extend_from_slice(&[
        "mcp:inbox.record:call",
        "mcp:outbox.enqueue:call",
        "inbox:rules:write",
    ]);
    let tok = token(&key, "user:ada", "acme", &caps);

    let body = r#"log("checking"); alert(#{ level: "critical", msg: "hot" });"#;
    let resp = router(gw)
        .oneshot(bearer(
            json_post("/rules/run", json!({ "body": body })),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let out: Value = json_body(resp).await;
    assert_eq!(out["output"]["kind"], "findings");

    let findings = out["findings"].as_array().unwrap();
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0]["level"], "critical");
    assert_eq!(findings[0]["data"]["alert"], true);

    let log = out["log"].as_array().unwrap();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0]["message"], "checking");

    // The budget readout is present (ms + ai calls/tokens).
    assert!(out["ms"].is_u64());
    assert_eq!(out["ai"]["calls"], 0);
    assert_eq!(out["ai"]["tokens"], 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_cage_error_is_400_with_the_verbatim_message() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:ada", "acme", CAPS);

    let resp = router(gw)
        .oneshot(bearer(
            json_post("/rules/run", json!({ "body": "eval(\"1 + 1\")" })),
            &tok,
        ))
        .await
        .unwrap();
    // The cage rejects `eval` — author feedback, shown not swallowed.
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let msg = body_text(resp).await;
    assert!(!msg.is_empty(), "the cage message is shown, not blank");
    assert_ne!(msg, "not permitted", "a cage error is NOT an opaque deny");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_ai_rule_with_no_model_is_400_ai_not_configured() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:ada", "acme", CAPS);

    let resp = router(gw)
        .oneshot(bearer(
            json_post("/rules/run", json!({ "body": "ai.complete(\"hi\")" })),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let msg = body_text(resp).await;
    assert!(
        msg.contains("AI not configured"),
        "the AI-not-configured state renders verbatim, got: {msg}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn running_a_missing_saved_rule_is_404() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:ada", "acme", CAPS);

    let resp = router(gw)
        .oneshot(bearer(
            json_post("/rules/run", json!({ "rule_id": "ghost" })),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interactive_channel_posts_append_with_distinct_ids_and_ascending_ts() {
    // Regression: interactive `rules.run` carried no `ts`, so the core's logical `now` collapsed to 0 —
    // every `channel.post` reused the deterministic id `rule-channel-0-1` (upserting the same row) and
    // stamped `ts: 0` (sorting as the oldest). The gateway now fills the live clock when the caller omits
    // `ts`, so two runs a tick apart append two distinct, ascending messages. A caller that DOES supply
    // its own `ts` still upserts deterministically (the scheduled/re-run idempotency contract).
    let node = Arc::new(Node::boot_as(NodeRole::Hub).await.expect("node boots"));
    let key = SigningKey::generate();
    let ws = "rules-chan";

    // The rule's channel handle re-enters the ONE MCP contract (`channel.post`/`history`), so the caller
    // needs both the MCP verb cap AND the channel `bus:chan/{cid}:{pub|sub}` gate the host fn re-checks.
    let mut caps: Vec<&str> = CAPS.to_vec();
    caps.extend_from_slice(&[
        "mcp:channel.post:call",
        "mcp:channel.history:call",
        "bus:chan/room:pub",
        "bus:chan/room:sub",
    ]);
    let tok = token(&key, "user:ada", ws, &caps);

    // The route has no `ts` field — the interactive client can't (and doesn't) send one, which is
    // exactly the bug's condition. So each run's `now` comes from its gateway's clock, injected by the
    // fix. We build one gateway per run with an advancing fixed clock (same node, so the store is
    // shared) — mimicking the live wall clock ticking between two Playground runs.
    let run = |clock: u64, body: &'static str| {
        let node = node.clone();
        let key = key.clone();
        let tok = tok.clone();
        async move {
            let gw = Gateway::new(node, key, clock);
            let resp = router(gw)
                .oneshot(bearer(json_post("/rules/run", json!({ "body": body })), &tok))
                .await
                .unwrap();
            if resp.status() != StatusCode::OK {
                let s = resp.status();
                panic!("run failed {s}: {}", body_text(resp).await);
            }
            json_body::<Value>(resp).await
        }
    };

    // Two separate interactive runs post one message each; a third read-only run reads the accumulated
    // history back. The clocks (999/1000) stay inside the token's validity window (`NOW-1..NOW+10_000`).
    let _ = run(999, r#"channel.post("room", #{ body: "first" })"#).await;
    let _ = run(1000, r#"channel.post("room", #{ body: "second" })"#).await;
    let out = run(1000, r#"channel.history("room", 0)"#).await;

    // The last run's history read holds BOTH messages — distinct deterministic ids and ascending ts,
    // not one silently-overwritten row at ts 0.
    // A rule whose last expression is a channel array renders as a scalar output holding the JSON array.
    let rows = out["output"]["value"]
        .as_array()
        .or_else(|| out["output"]["rows"].as_array())
        .expect("history rows");
    assert_eq!(rows.len(), 2, "two interactive posts append, not overwrite");

    let ids: Vec<&str> = rows.iter().map(|r| r["id"].as_str().unwrap()).collect();
    assert_ne!(ids[0], ids[1], "each run gets a distinct id");
    assert!(
        ids.iter().all(|id| !id.starts_with("rule-channel-0-")),
        "no id collapsed to the `now=0` deterministic id, got {ids:?}"
    );

    let ts: Vec<u64> = rows.iter().map(|r| r["ts"].as_u64().unwrap()).collect();
    assert!(ts.iter().all(|t| *t != 0), "no message stamped ts 0, got {ts:?}");
    assert!(ts[0] < ts[1], "messages sort by ascending ts, got {ts:?}");

    // The idempotency contract is intact: a run at the SAME clock re-derives the SAME id and upserts —
    // it does NOT grow the history. (A scheduled/programmatic re-run relies on this.)
    let _ = run(1000, r#"channel.post("room", #{ body: "second again" })"#).await;
    let after = run(1000, r#"channel.history("room", 0)"#).await;
    let after_rows = after["output"]["value"].as_array().expect("history rows");
    assert_eq!(
        after_rows.len(),
        2,
        "a same-clock re-run upserts (deterministic id) — no duplicate row"
    );
}

// ── one capability-deny test PER verb: a token holding every rules cap EXCEPT the one under test is
//    refused that route server-side (the gateway re-checks; the UI cap-gate is convenience only). ──

/// Build a token holding every rules cap (incl. the store surface caps) except `missing` — so the
/// only thing standing between the caller and the route is the one MCP cap under test.
fn caps_without(missing: &str) -> Vec<String> {
    CAPS.iter()
        .filter(|c| **c != missing)
        .map(|s| s.to_string())
        .collect()
}

async fn denied(missing: &str, req: axum::http::Request<axum::body::Body>) {
    let (gw, key) = gateway().await;
    let caps = caps_without(missing);
    let caps_ref: Vec<&str> = caps.iter().map(|s| s.as_str()).collect();
    let tok = token(&key, "user:ada", "acme", &caps_ref);
    let resp = router(gw).oneshot(bearer(req, &tok)).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "missing {missing} → 403"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn run_without_the_cap_is_denied() {
    denied(
        "mcp:rules.run:call",
        json_post("/rules/run", json!({ "body": "1" })),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn save_without_the_cap_is_denied() {
    denied(
        "mcp:rules.save:call",
        json_post("/rules", json!({ "id": "x", "body": "1" })),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn get_without_the_cap_is_denied() {
    denied("mcp:rules.get:call", get_req("/rules/x")).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn list_without_the_cap_is_denied() {
    denied("mcp:rules.list:call", get_req("/rules")).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn delete_without_the_cap_is_denied() {
    denied("mcp:rules.delete:call", delete_req("/rules/x")).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_sessions_are_workspace_isolated() {
    // One node, two sessions in different workspaces — ws-B sees none of ws-A's rules.
    let node = Arc::new(Node::boot_as(NodeRole::Hub).await.expect("node boots"));
    let key = SigningKey::generate();
    let ada = token(&key, "user:ada", "ws-a", CAPS);
    let ben = token(&key, "user:ben", "ws-b", CAPS);

    router(gateway_on(node.clone(), &key))
        .oneshot(bearer(
            json_post(
                "/rules",
                json!({ "id": "secret", "name": "A", "body": "1" }),
            ),
            &ada,
        ))
        .await
        .unwrap();

    // Ben (ws-B) gets a 404 for ws-A's rule and an empty roster.
    let resp = router(gateway_on(node.clone(), &key))
        .oneshot(bearer(get_req("/rules/secret"), &ben))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let resp = router(gateway_on(node.clone(), &key))
        .oneshot(bearer(get_req("/rules"), &ben))
        .await
        .unwrap();
    let body: Value = json_body(resp).await;
    assert!(body["rules"].as_array().unwrap().is_empty());
}
