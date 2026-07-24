//! Dashboard auto-capture + undo/redo (undo scope): `dashboard.save`/`dashboard.delete` are single-record
//! upserts at `dashboard:{id}` — the same reversible floor as `inbox.record`/`assets.put_doc` — so a save
//! or delete dispatched through the real `call_tool` seam is auto-journaled **undoable**, and `undo`
//! restores the prior stored record (a create's before-image is empty → undo removes it; a delete's
//! before-image is the live record → undo resurrects it). Exercised against a real booted node + in-memory
//! store through the actual MCP bridge (rule #9, no mocks).
//!
//! What is proven here:
//!   - `dashboard.save` lands an **undoable** journal entry automatically (before-image captured at the seam);
//!   - undo of an EDIT restores the prior cell set (not the post-edit one);
//!   - undo of a CREATE removes the dashboard (empty before-image), and redo re-creates it;
//!   - `dashboard.delete` is journaled undoable and undo **resurrects** the record.
//! These are exactly the flows the rubix-ai dashboard toolbar drives — the buttons gate on the resulting
//! `history.list` `undoable`/`redoable` flags, so this is the backend contract those buttons depend on.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    call_tool, dashboard_get, history_list, redo as host_redo, undo as host_undo, DashboardError,
    Node,
};

fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

/// Every cap the dashboard toolbar flow touches: save/delete/get to mutate + read back, history/undo/redo
/// to drive the buttons. Undo's no-escalation gate re-checks the ORIGINAL tool's cap, so the caller must
/// hold `dashboard.save`/`dashboard.delete` to undo them — which the owner does.
fn owner(ws: &str) -> Principal {
    principal(
        "user:owner",
        ws,
        &[
            "mcp:dashboard.save:call",
            "mcp:dashboard.delete:call",
            "mcp:dashboard.get:call",
            "mcp:history.list:call",
            "mcp:undo:call",
            "mcp:redo:call",
        ],
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn dashboard_save_is_auto_journaled_undoable() {
    let ws = "undo-dash-save";
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = owner(ws);

    call_tool(
        &node,
        &p,
        ws,
        "dashboard.save",
        r#"{"id":"ops","title":"Ops","cells":[],"now":1}"#,
    )
    .await
    .expect("dashboard.save dispatches");

    // Auto-captured as an UNDOABLE step, scoped to this dashboard's surface — no manual record_change.
    let items = history_list(&node.store, &p, ws, "user:owner", "ops")
        .await
        .expect("history reads");
    assert_eq!(items.len(), 1, "exactly one auto-captured step");
    assert_eq!(items[0].tool, "dashboard.save");
    assert!(
        items[0].undoable,
        "a dashboard save is auto-journaled undoable — this is what lights the toolbar undo button"
    );
    assert_eq!(items[0].class, lb_undo::Class::Reversible);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn undo_of_an_edit_restores_the_prior_cell_set() {
    let ws = "undo-dash-edit";
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = owner(ws);

    // Create with one cell, then edit to two cells (the "edit a widget" the issue describes).
    call_tool(
        &node,
        &p,
        ws,
        "dashboard.save",
        r#"{"id":"ops","title":"Ops","cells":[{"i":"a","x":0,"y":0,"w":4,"h":4}],"now":1}"#,
    )
    .await
    .expect("create dispatches");
    call_tool(
        &node,
        &p,
        ws,
        "dashboard.save",
        r#"{"id":"ops","title":"Ops","cells":[{"i":"a","x":0,"y":0,"w":4,"h":4},{"i":"b","x":4,"y":0,"w":4,"h":4}],"now":2}"#,
    )
    .await
    .expect("edit dispatches");

    // Undo the edit on this surface → the prior (one-cell) record is restored.
    host_undo(&node.store, &p, ws, "user:owner", "ops")
        .await
        .expect("undo the edit");
    let got = dashboard_get(&node.store, &p, ws, "ops")
        .await
        .expect("still present after undo");
    assert_eq!(
        got.cells.len(),
        1,
        "undo restored the pre-edit cell set (not the post-edit two-cell form)"
    );

    // Redo re-applies the edit.
    host_redo(&node.store, &p, ws, "user:owner", "ops")
        .await
        .expect("redo the edit");
    let got = dashboard_get(&node.store, &p, ws, "ops")
        .await
        .expect("present after redo");
    assert_eq!(got.cells.len(), 2, "redo re-applied the two-cell edit");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn undo_of_a_create_removes_the_dashboard_and_redo_recreates_it() {
    let ws = "undo-dash-create";
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = owner(ws);

    call_tool(
        &node,
        &p,
        ws,
        "dashboard.save",
        r#"{"id":"ops","title":"Ops","cells":[],"now":1}"#,
    )
    .await
    .expect("create dispatches");

    // Before-image of a create is empty → undo removes the record entirely.
    host_undo(&node.store, &p, ws, "user:owner", "ops")
        .await
        .expect("undo the create");
    assert!(
        matches!(
            dashboard_get(&node.store, &p, ws, "ops").await.unwrap_err(),
            DashboardError::NotFound
        ),
        "undo of a create removes the dashboard (empty before-image)"
    );

    // Redo re-creates it.
    host_redo(&node.store, &p, ws, "user:owner", "ops")
        .await
        .expect("redo the create");
    dashboard_get(&node.store, &p, ws, "ops")
        .await
        .expect("redo re-created the dashboard");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn delete_is_journaled_undoable_and_undo_resurrects() {
    let ws = "undo-dash-delete";
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = owner(ws);

    // Create with a cell, then delete (the "delete a widget/dashboard" the issue describes).
    call_tool(
        &node,
        &p,
        ws,
        "dashboard.save",
        r#"{"id":"ops","title":"Ops","cells":[{"i":"a","x":0,"y":0,"w":4,"h":4}],"now":1}"#,
    )
    .await
    .expect("create dispatches");
    call_tool(&node, &p, ws, "dashboard.delete", r#"{"id":"ops","now":2}"#)
        .await
        .expect("delete dispatches");

    // Gone.
    assert!(matches!(
        dashboard_get(&node.store, &p, ws, "ops").await.unwrap_err(),
        DashboardError::NotFound
    ));

    // The delete was journaled undoable.
    let items = history_list(&node.store, &p, ws, "user:owner", "ops")
        .await
        .expect("history reads");
    assert!(
        items
            .iter()
            .any(|i| i.tool == "dashboard.delete" && i.undoable),
        "the delete is auto-journaled undoable"
    );

    // Undo the delete → the record (with its cell) is resurrected.
    host_undo(&node.store, &p, ws, "user:owner", "ops")
        .await
        .expect("undo the delete");
    let got = dashboard_get(&node.store, &p, ws, "ops")
        .await
        .expect("resurrected after undo");
    assert_eq!(
        got.cells.len(),
        1,
        "undo of a delete resurrects the record with its prior cells"
    );
}
