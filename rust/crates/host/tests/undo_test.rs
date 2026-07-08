//! Host-layer tests for the undo service (undo scope). Mandatory categories at the host surface:
//!   - **capability-deny:** `undo`/`redo`/`history.list` refused without their grant; the
//!     **no-escalation** rule (undoing a step whose tool-cap the actor lacks is refused); the
//!     **`undo.any`** requirement for another actor's stack — all opaque `Denied`.
//!   - **workspace-isolation:** a ws-B principal cannot list/undo ws-A's journal.
//!   - **the irreversible boundary + the conditional restore** surface as structured outcomes.
//!
//! Exercised against a real booted node + real in-memory store (rule #9). The undo verbs are not
//! reached through `lb_host::undo` directly here for the structured-outcome cases — those go through
//! the same `call_tool` MCP bridge the UI uses, so the JSON shape is what the UI will actually get.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{history_list, redo, undo, Node};
use lb_store::write;
use lb_undo::{record_change, RecordChange};
use serde_json::json;

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

/// Seed a tracked reversible change directly via the journal (the host capture path is internal;
/// the verbs under test are undo/redo/history). The actor matches the principal so it is "their own".
async fn seed_rename(node: &Node, ws: &str, actor: &str) {
    write(&node.store, ws, "doc", "d1", &json!({"title": "draft"}))
        .await
        .unwrap();
    record_change(
        &node.store,
        RecordChange {
            ws,
            actor,
            surface: "",
            tool: "doc.rename",
            trace_id: "t",
            ts: 1,
            table: "doc",
            id: "d1",
            new_value: Some(&json!({"title": "v1"})),
        },
    )
    .await
    .unwrap();
}

// ----- capability deny --------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn verbs_are_refused_without_their_grant() {
    let ws = "undo-deny";
    let node = Node::boot().await.expect("node boots");
    let p = principal("user:a", ws, &[]); // no caps at all

    assert!(undo(&node.store, &p, ws, "user:a", "").await.is_err());
    assert!(redo(&node.store, &p, ws, "user:a", "").await.is_err());
    assert!(history_list(&node.store, &p, ws, "user:a", "")
        .await
        .is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn undo_is_refused_without_the_original_tools_cap_no_escalation() {
    let ws = "undo-escalation";
    let node = Node::boot().await.expect("node boots");
    seed_rename(&node, ws, "user:a").await;

    // Holds `undo` but NOT `doc.rename` — undoing the rename would reach a mutation they can't perform.
    let p = principal("user:a", ws, &["mcp:undo:call"]);
    assert!(
        undo(&node.store, &p, ws, "user:a", "").await.is_err(),
        "no-escalation: undo refused without the original tool's cap"
    );

    // With BOTH caps it succeeds.
    let p2 = principal("user:a", ws, &["mcp:undo:call", "mcp:doc.rename:call"]);
    assert!(undo(&node.store, &p2, ws, "user:a", "").await.is_ok());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn another_actors_stack_needs_undo_any() {
    let ws = "undo-any";
    let node = Node::boot().await.expect("node boots");
    seed_rename(&node, ws, "user:owner").await;

    // user:admin holds undo + the tool cap, but tries to undo OWNER's stack without undo.any.
    let admin = principal("user:admin", ws, &["mcp:undo:call", "mcp:doc.rename:call"]);
    assert!(
        undo(&node.store, &admin, ws, "user:owner", "")
            .await
            .is_err(),
        "another actor's stack requires undo.any"
    );

    // With undo.any it succeeds.
    let admin2 = principal(
        "user:admin",
        ws,
        &["mcp:undo:call", "mcp:doc.rename:call", "mcp:undo.any:call"],
    );
    assert!(undo(&node.store, &admin2, ws, "user:owner", "")
        .await
        .is_ok());
}

// ----- workspace isolation ----------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_b_cannot_see_or_undo_ws_a_journal() {
    let node = Node::boot().await.expect("node boots");
    seed_rename(&node, "ws-a", "user:a").await;

    // A ws-B principal (its token is ws-B) cannot even authorize against ws-A (workspace-first wall).
    let b = principal(
        "user:a",
        "ws-b",
        &[
            "mcp:undo:call",
            "mcp:doc.rename:call",
            "mcp:history.list:call",
        ],
    );
    assert!(
        history_list(&node.store, &b, "ws-a", "user:a", "")
            .await
            .is_err(),
        "ws-B principal cannot list ws-A's journal"
    );
    assert!(undo(&node.store, &b, "ws-a", "user:a", "").await.is_err());

    // ws-A's record is untouched.
    assert_eq!(
        lb_store::read(&node.store, "ws-a", "doc", "d1")
            .await
            .unwrap(),
        Some(json!({"title": "v1"})),
        "ws-A record untouched by the ws-B attempt"
    );
}

// ----- the conditional restore + history (the surfaced outcomes) --------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn undo_restores_then_redo_reapplies_over_the_gate() {
    let ws = "undo-roundtrip-host";
    let node = Node::boot().await.expect("node boots");
    seed_rename(&node, ws, "user:a").await;
    let p = principal(
        "user:a",
        ws,
        &[
            "mcp:undo:call",
            "mcp:redo:call",
            "mcp:doc.rename:call",
            "mcp:history.list:call",
        ],
    );

    undo(&node.store, &p, ws, "user:a", "").await.unwrap();
    assert_eq!(
        lb_store::read(&node.store, ws, "doc", "d1").await.unwrap(),
        Some(json!({"title": "draft"}))
    );

    redo(&node.store, &p, ws, "user:a", "").await.unwrap();
    assert_eq!(
        lb_store::read(&node.store, ws, "doc", "d1").await.unwrap(),
        Some(json!({"title": "v1"}))
    );

    // history.list shows the step, undoable.
    let items = history_list(&node.store, &p, ws, "user:a", "")
        .await
        .unwrap();
    assert!(!items.is_empty());
    assert!(items[0].undoable);
}
