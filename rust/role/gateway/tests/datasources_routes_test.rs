//! The datasource routes over the real gateway (rules-workbench scope, Phase 3) — the `datasource.*`
//! admin surface end to end. Mirrors `dashboard_routes_test.rs` at the transport boundary: the
//! add→list round-trip, capability-deny per verb, two-session workspace isolation, the DSN-redaction
//! assertion (the list body never contains the submitted DSN), and remove. The gateway re-checks every
//! cap server-side via `call_tool`; the workspace + principal come from the token (§7).
//!
//! `datasource.test` (the connectivity probe) needs the supervised federation sidecar — its real green
//! path is exercised in `crates/host/tests/federation_test.rs` against a spawned Postgres. Here, with
//! no sidecar installed, the probe surfaces a sidecar fault as `Extension` → a non-`200` (an HONEST RED
//! probe), and the page renders red. We assert the route fails honestly — never a fabricated green.

mod common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::*;
use lb_auth::SigningKey;
use lb_host::{Node, Role as NodeRole};
use lb_role_gateway::router;
use serde_json::{json, Value};
use tower::ServiceExt; // for `oneshot`

const CAPS: &[&str] = &[
    "mcp:datasource.add:call",
    "mcp:datasource.remove:call",
    "mcp:datasource.list:call",
    "mcp:datasource.test:call",
    // The DSN write at `datasource.add` is mediated into lb-secrets under the admin's authority — the
    // host requires this alongside the mcp cap (the dev login carries it). The per-verb deny tests
    // below strip the *mcp* cap, which is the capability boundary this slice gates.
    "secret:federation/*:write",
];

/// The DSN we submit on Add — the secret material that must NEVER appear in any response body.
const DSN: &str =
    "host=127.0.0.1 port=5432 user=lb password=SUPERSECRETpw dbname=fed sslmode=disable";

fn add_body() -> Value {
    json!({
        "name": "timescale",
        "kind": "postgres",
        "endpoint": "tsdb.acme:5432",
        "dsn": DSN,
    })
}

/// Regression (`dsn` was a required field on the `POST /datasources` body): a file-backed source
/// (a sqlite `endpoint` that IS the db path) carries no separate DSN, and the host's `datasource.add`
/// treats `dsn` as `Option`. Requiring it at the gateway made axum reject a DSN-less add with a `422`
/// **before the handler ran** — surfacing in the UI as "add datasource denied". This asserts a
/// DSN-less body now round-trips (200/ok) and the source lists. Before the fix: 422.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn add_without_a_dsn_is_accepted_over_the_gateway() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:ada", "acme", CAPS);

    let body = json!({
        "name": "buildings",
        "kind": "sqlite",
        "endpoint": "127.0.0.1:0",
        // no "dsn" — the sqlite file path lives in `endpoint`/is picked separately; the field is absent.
    });
    let resp = router(gw.clone())
        .oneshot(bearer(json_post("/datasources", body), &tok))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "a DSN-less add must be accepted (was 422 when `dsn` was required)"
    );
    let added: Value = json_body(resp).await;
    assert_eq!(added["ok"], true);

    // And it lists (the record persisted).
    let list = router(gw.clone())
        .oneshot(bearer(get_req("/datasources"), &tok))
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::OK);
    let body: Value = json_body(list).await;
    let names: Vec<&str> = body["datasources"]
        .as_array()
        .expect("datasources array")
        .iter()
        .filter_map(|s| s["name"].as_str())
        .collect();
    assert!(
        names.contains(&"buildings"),
        "the DSN-less source persisted (got {names:?})"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn add_then_list_round_trip_over_the_gateway() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:ada", "acme", CAPS);

    // add → ok
    let resp = router(gw.clone())
        .oneshot(bearer(json_post("/datasources", add_body()), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "add returns 200/ok");
    let added: Value = json_body(resp).await;
    assert_eq!(added["ok"], true);

    // list shows it — name + kind + endpoint + a secret ref, NEVER the DSN.
    let resp = router(gw.clone())
        .oneshot(bearer(get_req("/datasources"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = json_body(resp).await;
    let sources = body["datasources"].as_array().expect("datasources array");
    assert_eq!(sources.len(), 1, "the one registered source: {body}");
    let s = &sources[0];
    assert_eq!(s["name"], "timescale");
    assert_eq!(s["kind"], "postgres");
    assert_eq!(s["endpoint"], "tsdb.acme:5432");
    assert!(s.get("secret_ref").is_some(), "list shows the ref: {s}");

    // REDACTION: the serialized list body never contains the submitted DSN (or its password).
    let raw = body.to_string();
    assert!(
        !raw.contains(DSN) && !raw.contains("SUPERSECRETpw"),
        "datasource.list leaked the DSN: {raw}"
    );
    // No `dsn` key on any summary, ever.
    assert!(s.get("dsn").is_none(), "no dsn field on a summary: {s}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn remove_drops_the_source() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:ada", "acme", CAPS);

    router(gw.clone())
        .oneshot(bearer(json_post("/datasources", add_body()), &tok))
        .await
        .unwrap();

    let resp = router(gw.clone())
        .oneshot(bearer(delete_req("/datasources/timescale"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT, "remove → 204");

    let resp = router(gw.clone())
        .oneshot(bearer(get_req("/datasources"), &tok))
        .await
        .unwrap();
    let body: Value = json_body(resp).await;
    assert!(
        body["datasources"].as_array().unwrap().is_empty(),
        "roster empty after remove: {body}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn each_verb_is_denied_without_its_cap() {
    // For each verb, hold every datasource cap EXCEPT that one → the route is 403 server-side.
    let cases: &[(&str, &str, bool)] = &[
        ("mcp:datasource.add:call", "add", false),
        ("mcp:datasource.list:call", "list", false),
        ("mcp:datasource.remove:call", "remove", false),
        ("mcp:datasource.test:call", "test", false),
    ];
    for (missing, verb, _) in cases {
        let (gw, key) = gateway().await;
        let caps: Vec<&str> = CAPS.iter().copied().filter(|c| c != missing).collect();
        let tok = token(&key, "user:ada", "acme", &caps);

        let req = match *verb {
            "add" => bearer(json_post("/datasources", add_body()), &tok),
            "list" => bearer(get_req("/datasources"), &tok),
            "remove" => bearer(delete_req("/datasources/timescale"), &tok),
            "test" => bearer(json_post("/datasources/timescale/test", json!({})), &tok),
            _ => unreachable!(),
        };
        let resp = router(gw).oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::FORBIDDEN,
            "{verb} without {missing} → 403"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn two_sessions_are_workspace_isolated() {
    // One node, two sessions in different workspaces — ws-B sees none of ws-A's datasources.
    let node = Arc::new(Node::boot_as(NodeRole::Hub).await.expect("node boots"));
    let key = SigningKey::generate();
    let ada = token(&key, "user:ada", "ws-a", CAPS);
    let ben = token(&key, "user:ben", "ws-b", CAPS);

    router(gateway_on(node.clone(), &key))
        .oneshot(bearer(json_post("/datasources", add_body()), &ada))
        .await
        .unwrap();

    // Ben (ws-B) sees an empty roster — and certainly never the DSN.
    let resp = router(gateway_on(node.clone(), &key))
        .oneshot(bearer(get_req("/datasources"), &ben))
        .await
        .unwrap();
    let body: Value = json_body(resp).await;
    assert!(
        body["datasources"].as_array().unwrap().is_empty(),
        "ws-B roster is empty (the hard wall): {body}"
    );
    assert!(!body.to_string().contains("SUPERSECRETpw"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_probe_without_a_sidecar_is_an_honest_red() {
    // The connectivity probe with NO federation sidecar installed surfaces a real sidecar fault, NOT a
    // fabricated green. The green path is proven against a spawned Postgres in federation_test.rs; here
    // we assert the route fails honestly (a non-200 → the page renders RED). It is never 200/ok.
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:ada", "acme", CAPS);

    router(gw.clone())
        .oneshot(bearer(json_post("/datasources", add_body()), &tok))
        .await
        .unwrap();

    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post("/datasources/timescale/test", json!({})),
            &tok,
        ))
        .await
        .unwrap();
    assert_ne!(
        resp.status(),
        StatusCode::OK,
        "no sidecar → the probe is an honest non-200 (RED), never a fabricated green"
    );
}
