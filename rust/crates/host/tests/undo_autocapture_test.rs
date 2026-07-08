//! Auto-capture-on-dispatch (undo scope): EVERY mutating tool call is journaled automatically at the
//! dispatch seam, with its class derived from **runtime outbox taint** — no extension calls
//! `record_change` itself. Exercised through the real `call_tool` MCP bridge against a real booted
//! node + in-memory store (rule #9, no mocks). Mandatory categories included: capability-deny and
//! workspace-isolation of the auto-captured journal.
//!
//! What is proven here:
//!   - a reversible single-record mutation (`inbox.record`) lands an **undoable** journal entry
//!     automatically (before-image captured at the seam);
//!   - a call that reaches the outbox (`outbox.enqueue`) is auto-classified **irreversible** and
//!     journaled **not-undoable** (taint, not metadata);
//!   - capture does not require the caller to hold any undo cap (capture is host-internal), but
//!     reading the resulting journal still goes through the gated `history.list`;
//!   - the auto-captured journal is workspace-walled (a ws-B reader cannot see ws-A's entries).

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, history_list, Node};

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

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_reversible_mutation_is_auto_journaled_undoable() {
    let ws = "autocap-reversible";
    let node = Arc::new(Node::boot().await.expect("node boots"));
    // The caller needs the tool cap + history.list to read back; NO undo cap is needed to CAPTURE.
    let p = principal(
        "user:a",
        ws,
        &["mcp:inbox.record:call", "mcp:history.list:call"],
    );

    // A plain reversible single-record upsert through the dispatch seam.
    call_tool(
        &node,
        &p,
        ws,
        "inbox.record",
        r#"{"channel":"general","id":"m1","body":"hello","ts":1}"#,
    )
    .await
    .expect("inbox.record dispatches");

    // It was auto-captured as an UNDOABLE step — no manual record_change anywhere.
    let items = history_list(&node.store, &p, ws, "user:a", "")
        .await
        .expect("history reads");
    assert_eq!(items.len(), 1, "exactly one auto-captured step");
    assert_eq!(items[0].tool, "inbox.record");
    assert!(
        items[0].undoable,
        "a reversible mutation is auto-journaled undoable"
    );
    assert_eq!(items[0].class, lb_undo::Class::Reversible);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_outbox_call_is_auto_classified_irreversible() {
    let ws = "autocap-irreversible";
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal(
        "user:a",
        ws,
        &["mcp:outbox.enqueue:call", "mcp:history.list:call"],
    );

    // This reaches the outbox (external motion) → tainted irreversible at RUNTIME.
    call_tool(
        &node,
        &p,
        ws,
        "outbox.enqueue",
        r#"{"id":"e1","target":"github","action":"open_pr","payload":"{}","ts":1}"#,
    )
    .await
    .expect("outbox.enqueue dispatches");

    let items = history_list(&node.store, &p, ws, "user:a", "")
        .await
        .expect("history reads");
    assert_eq!(items.len(), 1, "the outbox call is journaled (as a marker)");
    assert_eq!(items[0].tool, "outbox.enqueue");
    assert!(
        !items[0].undoable,
        "reaching the outbox auto-classifies the step not-undoable (taint, not metadata)"
    );
    assert_eq!(items[0].class, lb_undo::Class::Irreversible);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_denied_call_is_not_journaled() {
    // MANDATORY capability-deny: a call the principal cannot make is refused BEFORE it mutates, so
    // nothing is auto-captured. (Reading needs history.list, which this principal does hold.)
    let ws = "autocap-deny";
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:a", ws, &["mcp:history.list:call"]); // NO inbox.record cap

    let denied = call_tool(
        &node,
        &p,
        ws,
        "inbox.record",
        r#"{"channel":"general","id":"m1","body":"x","ts":1}"#,
    )
    .await;
    assert!(denied.is_err(), "ungranted mutation is refused");

    let items = history_list(&node.store, &p, ws, "user:a", "")
        .await
        .expect("history reads");
    assert!(
        items.is_empty(),
        "a denied call mutates nothing and is not auto-journaled"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_auto_captured_journal_is_workspace_walled() {
    // MANDATORY workspace-isolation: an auto-captured entry in ws-A is invisible to a ws-B reader.
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let a = principal(
        "user:a",
        "ws-a",
        &["mcp:inbox.record:call", "mcp:history.list:call"],
    );
    call_tool(
        &node,
        &a,
        "ws-a",
        "inbox.record",
        r#"{"channel":"general","id":"m1","body":"a-only","ts":1}"#,
    )
    .await
    .expect("ws-a records");

    // ws-A actually has the entry.
    assert_eq!(
        history_list(&node.store, &a, "ws-a", "user:a", "")
            .await
            .unwrap()
            .len(),
        1
    );

    // A ws-B principal (token scoped to ws-B) cannot read ws-A's journal — workspace-first wall.
    let b = principal("user:a", "ws-b", &["mcp:history.list:call"]);
    assert!(
        history_list(&node.store, &b, "ws-a", "user:a", "")
            .await
            .is_err(),
        "ws-B principal cannot read ws-A's auto-captured journal"
    );
    // And ws-B's own stack is empty.
    assert!(history_list(&node.store, &b, "ws-b", "user:a", "")
        .await
        .unwrap()
        .is_empty());
}
