//! Gateway tests for `POST /workspaces/{ws}/provision` + `POST /workspaces/{ws}/reconcile`
//! (workspace-provision scope, NubeDev/lb#121): the admin path succeeds with a report-shaped reply
//! that carries **no token** (the caller's session is untouched — the whole point of the verb), a
//! plain member is denied with zero residue, and reconcile repairs a hand-crafted orphan over HTTP.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{bearer, gateway, json_body, token};
use lb_role_gateway::router;
use serde_json::{json, Value};
use tower::ServiceExt;

fn json_post(uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

const ADMIN_CAPS: &[&str] = &[
    "mcp:workspace.provision:call",
    "mcp:workspace.reconcile:call",
    "mcp:workspace.list:call",
];

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn provision_route_returns_report_with_no_token() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:test", "nube", ADMIN_CAPS);

    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/workspaces/other-ws/provision",
                json!({ "name": "Other Co", "admin": "user:alice" }),
            ),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let report: Value = json_body(resp).await;
    assert_eq!(report["record"]["ws"], "other-ws");
    assert_eq!(report["admin_sub"], "user:alice");
    assert!(report["roles_granted"].as_array().is_some());
    // The headline guarantee: the reply carries NO token anywhere — the caller keeps their session.
    let raw = report.to_string();
    assert!(
        report.get("token").is_none() && !raw.contains("\"token\""),
        "the provision reply must not carry a token: {raw}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn provision_route_denies_a_member_with_zero_residue() {
    let (gw, key) = gateway().await;
    let member = token(&key, "user:bob", "nube", &["mcp:workspace.list:call"]);

    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/workspaces/other-ws/provision",
                json!({ "name": "Other Co" }),
            ),
            &member,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // Zero residue: the directory does not list it.
    let resp = router(gw.clone())
        .oneshot(bearer(
            Request::builder()
                .method("GET")
                .uri("/workspaces")
                .body(Body::empty())
                .unwrap(),
            &member,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let listed: Vec<Value> = json_body(resp).await;
    assert!(!listed.iter().any(|w| w["ws"] == "other-ws"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn reconcile_route_repairs_an_orphan() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:test", "nube", ADMIN_CAPS);

    // Hand-craft the orphan the old create path could leave: directory row, no membership.
    lb_host::workspace_register(&gw.node.store, "other-ws", "Other Co", 1)
        .await
        .unwrap();

    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post("/workspaces/other-ws/reconcile", json!({})),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let report: Value = json_body(resp).await;
    assert_eq!(report["admin_sub"], "user:test");
    assert!(report["fixed"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f == "membership"));
    let roster = lb_host::login_workspaces(&gw.node.store, "user:test")
        .await
        .unwrap();
    assert!(roster.iter().any(|w| w.ws == "other-ws"));
}
