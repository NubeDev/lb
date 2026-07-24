//! Regression (undo-exposure scope): the dashboard REST routes must go through the `call_tool` seam so
//! the undo auto-capture fires — a save/delete driven by the browser over `POST /dashboards` /
//! `DELETE /dashboards/{id}` is journalled on the dashboard's surface, exactly like the same verb over
//! `/mcp/call`. Before the fix these routes called `lb_host::dashboard_save_meta`/`dashboard_delete`
//! DIRECTLY, bypassing `capture_dispatch`, so browser edits were never captured and the undo toolbar
//! stayed dark no matter what the store held. This pins the routes to the seam.
//!
//! Read back through the real host `history_list` on `surface = <dashboard id>` (the same surface the
//! UI's `history.list`/`undo`/`redo` use), so the assertion is the exact stack the toolbar reads.

mod common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::*;
use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{history_list, Node};
use lb_role_gateway::router;
use serde_json::json;
use tower::ServiceExt; // for `oneshot`

const CAPS: &[&str] = &[
    "mcp:dashboard.get:call",
    "mcp:dashboard.save:call",
    "mcp:dashboard.delete:call",
    "mcp:history.list:call",
    "mcp:undo:call",
    "mcp:redo:call",
];

/// A `user:ada`/`acme` principal that can read the journal — verifying a freshly-minted token gives
/// the same `Principal` shape the gateway's own `authenticate` produces.
fn reader(key: &SigningKey) -> Principal {
    let claims = Claims {
        sub: "user:ada".into(),
        ws: "acme".into(),
        role: Role::Member,
        caps: vec!["mcp:history.list:call".into()],
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    let tok = mint(key, &claims);
    verify(key, &tok, 1).expect("token verifies")
}

/// Count the undoable steps on `surface` for `user:ada`'s own stack in `acme`.
async fn undoable_on(node: &Arc<Node>, key: &SigningKey, surface: &str) -> usize {
    let p = reader(key);
    let items = history_list(&node.store, &p, "acme", "user:ada", surface)
        .await
        .expect("history reads");
    items.iter().filter(|i| i.undoable).count()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_rest_save_is_auto_captured_undoable_on_the_dashboard_surface() {
    let (gw, key) = gateway().await;
    let node = gw.node.clone();
    let tok = token(&key, "user:ada", "acme", CAPS);

    // Create over the REST route (the UI's `dashboard_save` transport).
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/dashboards",
                json!({ "id": "ops", "title": "Ops", "cells": [] }),
            ),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // The save is journalled UNDOABLE on the dashboard's surface — this is what lights the toolbar undo
    // button. (Pre-fix: 0 — the direct host-fn call bypassed capture.)
    assert_eq!(
        undoable_on(&node, &key, "ops").await,
        1,
        "a REST dashboard save must be auto-captured undoable on its surface"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_rest_delete_is_auto_captured_undoable() {
    let (gw, key) = gateway().await;
    let node = gw.node.clone();
    let tok = token(&key, "user:ada", "acme", CAPS);

    // Create, then delete — both over the REST routes.
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/dashboards",
                json!({ "id": "ops", "title": "Ops", "cells": [] }),
            ),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = router(gw.clone())
        .oneshot(bearer(delete_req("/dashboards/ops"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Both the save and the delete are journalled undoable on the surface (undo of the delete would
    // resurrect the record; undo of the save would remove it).
    assert!(
        undoable_on(&node, &key, "ops").await >= 2,
        "a REST dashboard delete must be auto-captured undoable on its surface"
    );
}
