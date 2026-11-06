//! `schedule.list` and `schedule.save` through the real MCP tool path (`call_tool`), so the wire the
//! UI and the extension read is pinned: `rev_checked`, `unreadable`, and the `{id, rev}` answer.

use std::sync::Arc;

use lb_host::{call_tool, Node};
use lb_store::write;
use serde_json::json;

use super::support::{offices_at, principal, READ, WRITE};

const WS: &str = "ws-tool-path";
const CAPS: &[&str] = &[
    "mcp:schedule.list:call",
    "mcp:schedule.save:call",
    READ,
    WRITE,
];

/// A tool answer is JSON text; parse it.
fn parsed(answer: String) -> serde_json::Value {
    serde_json::from_str(&answer).expect("tool answers JSON")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn list_and_save_answer_the_documented_wire() {
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal(WS, CAPS);

    // An empty workspace still answers the version floor.
    let empty = parsed(
        call_tool(&node, &p, WS, "schedule.list", "{}")
            .await
            .unwrap(),
    );
    assert_eq!(
        empty,
        json!({ "schedules": [], "unreadable": [], "rev_checked": true })
    );

    // An old-shape row is named under `unreadable` with its rev.
    let old = json!({ "id": "legacy", "weekly": [], "exceptions": [] });
    write(&node.store, WS, "schedule", "legacy", &old)
        .await
        .unwrap();
    let listed = parsed(
        call_tool(&node, &p, WS, "schedule.list", "{}")
            .await
            .unwrap(),
    );
    assert_eq!(listed["rev_checked"], json!(true));
    assert_eq!(listed["schedules"], json!([]));
    assert_eq!(listed["unreadable"], json!([{ "id": "legacy", "rev": 1 }]));

    // A save answers exactly `{id, rev}`.
    let body = offices_at(0).to_string();
    let saved = parsed(
        call_tool(&node, &p, WS, "schedule.save", &body)
            .await
            .unwrap(),
    );
    assert_eq!(saved, json!({ "id": "office-hours", "rev": 1 }));
}
