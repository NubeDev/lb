//! Regression: the retired `chains.*` verbs are GONE, not merely ungranted (chains-retirement scope,
//! the headline). `flows` is the one DAG engine; a client that still hard-codes `chains.run` must get
//! the host's unknown-verb refusal, never a silent 500 or a partial execution.
//!
//! The subtlety the scope calls out: an UNGRANTED caller is denied opaquely at the authorize gate
//! (`Denied`) — that can't distinguish "gone" from "locked". To prove the verb is *gone*, we grant the
//! now-defunct `mcp:chains.<verb>:call` so the caller PASSES the gate and reaches dispatch, where the
//! deleted verb resolves to nothing (no host-native arm, no such extension in the registry) →
//! `NotFound`. That `NotFound` — reachable only by a caller holding a cap for a verb that no longer
//! exists — is the guard against a stray re-add. Real store, real caps, real dispatch — no mocks.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, Node};
use lb_mcp::ToolError;

fn principal(ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "user:test".into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
    };
    verify(&key, &mint(&key, &claims), 1).unwrap()
}

/// Every retired chain verb + its (defunct) grant + a representative input.
const RETIRED: &[(&str, &str)] = &[
    ("chains.save", r#"{"id":"c","name":"c","steps":[]}"#),
    ("chains.run", r#"{"chain_id":"c"}"#),
    ("chains.get", r#"{"id":"c"}"#),
    ("chains.list", "{}"),
    ("chains.runs.get", r#"{"id":"c","run_id":"r"}"#),
    ("chains.delete", r#"{"id":"c"}"#),
];

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn every_retired_chains_verb_is_unknown_not_just_ungranted() {
    let ws = "chains-retired";
    let node = Arc::new(Node::boot().await.unwrap());
    // Grant the defunct caps so authorization PASSES — the refusal we assert is dispatch-level
    // (`NotFound`, the verb is gone), not the authorize gate (`Denied`, which every caller hits).
    let caps: Vec<&str> = RETIRED
        .iter()
        .map(|(v, _)| Box::leak(format!("mcp:{v}:call").into_boxed_str()) as &str)
        .collect();
    let p = principal(ws, &caps);

    for (verb, input) in RETIRED {
        let err = call_tool(&node, &p, ws, verb, input)
            .await
            .expect_err("a retired verb must not dispatch");
        assert!(
            matches!(err, ToolError::NotFound),
            "{verb} must be NotFound (gone), not {err:?} — a `Denied` here would mean the verb still \
             exists but is locked; the retirement requires it be UNROUTABLE"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_ungranted_retired_verb_stays_opaque_denied() {
    // The complement: WITHOUT the (defunct) grant, the authorize gate refuses opaquely — the MCP
    // contract never leaks verb existence. This documents why the test above must grant the cap to
    // reach the `NotFound` proof.
    let ws = "chains-retired-nogrant";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal(ws, &[]);
    let err = call_tool(&node, &p, ws, "chains.run", r#"{"chain_id":"c"}"#)
        .await
        .expect_err("no grant → refused");
    assert!(matches!(err, ToolError::Denied), "opaque, got {err:?}");
}
