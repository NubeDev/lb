//! The **caveat stamp** and the **workspace category vocabulary**
//! (`docs/scope/insights/case-plane-scope.md` §"Data model" + §"Vocabulary").
//!
//! Two facts land together here because they read the same store row:
//!
//!   - **`category` is validated against the workspace's own declared set.** lb ships NO default
//!     value list (rule 10) — an unseeded workspace validates nothing, and a seeded one rejects a
//!     value outside its declared set, naming the set.
//!   - **A finding derived from data that is itself under question is caveated**, and a caveated
//!     finding **never breaks through**: no immediate delivery, while the identical uncaveated
//!     raise delivers. Resolve the gating finding and the next raise comes back clean.
//!
//! Every category value in this file is seeded BY THIS TEST as workspace data. That is the point:
//! grep `rust/crates/` for one of them and you will find it only here, never in the crate.
//!
//! Real booted `Node`: real store (`mem://`), real tag graph, real caps, the real `call_tool`
//! bridge, and real channel deliveries read back through the real inbox. No mocks (CLAUDE §9).
//!
//! **Fixtures only.** Split out of `insight_caveat_test.rs` when that file passed the 400-line
//! FILE-LAYOUT limit. The tests live in the `caveat_*.rs` siblings, aggregated by
//! `case_suite.rs`; this module holds what they share and asserts nothing itself.

#![allow(dead_code)]

pub use std::sync::Arc;

pub use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
pub use lb_host::{call_tool, Node};
pub use lb_mcp::ToolError;
pub use serde_json::{json, Value};

pub const RAISE: &str = "mcp:insight.raise:call";
pub const GET: &str = "mcp:insight.get:call";
pub const LIST: &str = "mcp:insight.list:call";
pub const RESOLVE: &str = "mcp:insight.resolve:call";
pub const SUB_CREATE: &str = "mcp:insight.sub.create:call";
pub const CHAN_PUB: &str = "bus:chan/*:pub";
pub const INBOX_LIST: &str = "mcp:inbox.list:call";

/// The workspace's declared vocabulary, seeded by this test the way a pack would seed it. These
/// strings are DATA — they exist in this file and nowhere in the crate.
pub const VALUES: [&str; 3] = ["dq", "device", "optimisation"];
/// The value this workspace declares as gating. Named `dq` rather than anything product-shaped so
/// no reader mistakes it for a vocabulary lb knows.
pub const GATE: &str = "dq";

pub fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
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

pub fn caps() -> Vec<&'static str> {
    vec![RAISE, GET, LIST, RESOLVE, SUB_CREATE, CHAN_PUB, INBOX_LIST]
}

pub async fn call(
    node: &Arc<Node>,
    p: &Principal,
    ws: &str,
    tool: &str,
    input: Value,
) -> Result<Value, ToolError> {
    let out = call_tool(node, p, ws, tool, &input.to_string()).await?;
    Ok(serde_json::from_str(&out).unwrap())
}

/// Seed the `tag_vocab:category` row a pack would install. Written straight to the store because
/// the vocabulary is workspace CONFIG, not something the insight surface mints.
pub async fn seed_vocab(node: &Arc<Node>, ws: &str) {
    lb_store::write(
        &node.store,
        ws,
        "tag_vocab",
        "category",
        &json!({ "key": "category", "values": VALUES, "gates": [GATE] }),
    )
    .await
    .expect("vocab seeded");
}

/// A raise carrying an optional category and optional evidence subjects.
pub fn raise_input(
    dedup_key: &str,
    severity: &str,
    ts: u64,
    category: Option<&str>,
    subjects: &[&str],
) -> Value {
    let mut input = json!({
        "dedup_key": dedup_key,
        "severity": severity,
        "title": format!("finding {dedup_key}"),
        "origin": { "kind": "rule", "ref": "rule:r1" },
        "ts": ts,
    });
    if let Some(c) = category {
        input["tags"] = json!({ "category": c });
    }
    if !subjects.is_empty() {
        input["evidence"] = json!({ "source": "demo", "subjects": subjects });
    }
    input
}

pub async fn raise(
    node: &Arc<Node>,
    p: &Principal,
    ws: &str,
    input: Value,
) -> Result<Value, ToolError> {
    call(node, p, ws, "insight.raise", input).await
}
