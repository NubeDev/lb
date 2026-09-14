//! A board's NAMED QUERIES must survive a save (shared-queries scope).
//!
//! This is the one test that has to exist. `Dashboard` DROPS unknown top-level keys, so an untyped
//! `queries` block behaves perfectly in memory, renders correctly, and vanishes on the first save —
//! the author configures a board, watches it work, reloads, and it is gone, with no error anywhere.
//! Nothing else in the feature can be trusted until the round-trip is pinned.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, Node};
use lb_mcp::ToolError;
use serde_json::{json, Value};

const SAVE: &str = "mcp:dashboard.save:call";
const GET: &str = "mcp:dashboard.get:call";

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
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

async fn call(
    node: &Arc<Node>,
    p: &Principal,
    ws: &str,
    tool: &str,
    input: Value,
) -> Result<Value, ToolError> {
    let out = call_tool(node, p, ws, tool, &input.to_string()).await?;
    Ok(serde_json::from_str(&out).unwrap())
}

/// The shipped Insights board, as it would be authored with shared queries.
fn alerts_query() -> Value {
    json!({
        "alerts": {
            "tool": "insight.list",
            "args": { "tags": { "kind": "alert" }, "limit": 200, "counts": true }
        }
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn named_queries_survive_a_save_and_come_back_on_get() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:test", "ws-a", &[SAVE, GET]);

    call(
        &node,
        &p,
        "ws-a",
        "dashboard.save",
        json!({ "id": "board", "title": "Board", "cells": [], "queries": alerts_query(), "now": 1 }),
    )
    .await
    .expect("save");

    let got = call(&node, &p, "ws-a", "dashboard.get", json!({ "id": "board" }))
        .await
        .expect("get");
    assert_eq!(
        got["queries"],
        alerts_query(),
        "the block must round-trip verbatim"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_layout_save_preserves_the_queries_it_does_not_mention() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:test", "ws-a", &[SAVE, GET]);

    call(
        &node,
        &p,
        "ws-a",
        "dashboard.save",
        json!({ "id": "b", "title": "B", "cells": [], "queries": alerts_query(), "now": 1 }),
    )
    .await
    .expect("save with queries");

    // A layout save — the common case — omits `queries` entirely. Blanking the block here would mean
    // dragging one panel silently deletes the board's data wiring.
    call(
        &node,
        &p,
        "ws-a",
        "dashboard.save",
        json!({ "id": "b", "title": "B", "cells": [], "now": 2 }),
    )
    .await
    .expect("layout save");

    let got = call(&node, &p, "ws-a", "dashboard.get", json!({ "id": "b" }))
        .await
        .expect("get");
    assert_eq!(
        got["queries"],
        alerts_query(),
        "omitting the key must PRESERVE, never clear"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_empty_object_clears_the_queries() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:test", "ws-a", &[SAVE, GET]);

    call(
        &node,
        &p,
        "ws-a",
        "dashboard.save",
        json!({ "id": "c", "title": "C", "cells": [], "queries": alerts_query(), "now": 1 }),
    )
    .await
    .expect("save");
    // An author must be able to remove the last one — the `reportIds` precedent. Without this,
    // preserve-on-omit would make the block permanent.
    call(
        &node,
        &p,
        "ws-a",
        "dashboard.save",
        json!({ "id": "c", "title": "C", "cells": [], "queries": {}, "now": 2 }),
    )
    .await
    .expect("clear");

    let got = call(&node, &p, "ws-a", "dashboard.get", json!({ "id": "c" }))
        .await
        .expect("get");
    assert_eq!(got["queries"], json!({}), "an empty object clears");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_board_authored_before_the_field_reads_as_no_queries() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:test", "ws-a", &[SAVE, GET]);
    call(
        &node,
        &p,
        "ws-a",
        "dashboard.save",
        json!({ "id": "old", "title": "Old", "cells": [], "now": 1 }),
    )
    .await
    .expect("save");
    let got = call(&node, &p, "ws-a", "dashboard.get", json!({ "id": "old" }))
        .await
        .expect("get");
    assert_eq!(
        got["queries"],
        json!({}),
        "additive: every existing board is unchanged"
    );
}
