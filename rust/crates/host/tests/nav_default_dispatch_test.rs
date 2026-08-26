//! `nav.get_default` over the REAL MCP dispatcher — the cap-alias tripwire (nav scope).
//!
//! The verb's own gate, round-trip and workspace wall are `nav_test.rs`; the gateway route is
//! `role/gateway/tests/nav_default_route_test.rs`. Neither crosses `tool_gate::gate_tool_for`, and
//! that is the failure this file exists for: a verb is gated on its own namesake by default, and
//! `mcp:nav.get_default:call` exists in NO role bundle — so without the alias arm pointing it at
//! `nav.resolve`, the verb refuses EVERY caller over the dispatcher (admins included) while every
//! direct-call test stays green. That is the shipped-but-unusable shape recorded in `tool_gate.rs`.
//!
//! The measurement from `nav_ext_boards_gate_test.rs` applies verbatim here: the outer gate answers
//! a bare `Denied`, not `NotFound`, when the alias is missing — `nav.` is a known host-native family,
//! so the dispatch arm IS reached and only the cap question is wrong. "Correctly denied" and "alias
//! missing" are the same value, so a deny assertion cannot tell them apart. **The POSITIVE test is
//! the tripwire**: delete the `nav.get_default` arm in `gate_tool_for` and
//! `a_member_reads_the_default_over_the_dispatcher` fails, and nothing else does.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, nav_save, nav_set_default, Node};
use lb_mcp::ToolError;
use serde_json::{json, Value};

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

const SAVE: &str = "mcp:nav.save:call";
const RESOLVE: &str = "mcp:nav.resolve:call";

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

/// **THE GATE-ALIAS TRIPWIRE.** A plain member holding ONLY `mcp:nav.resolve:call` — and no
/// authoring cap, and no `mcp:nav.get_default:call`, which exists in no bundle — reads the pointer
/// over the dispatcher. This is the member-level half of the read's whole point: the default shapes
/// their menu, so they may ask which nav it names.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_member_reads_the_default_over_the_dispatcher() {
    let ws = "ws-nav-default-dispatch";
    let node = Arc::new(Node::boot().await.unwrap());
    let admin = principal("user:ada", ws, &[SAVE, RESOLVE]);
    nav_save(&node.store, &admin, ws, "ops", "Ops", vec![], 1)
        .await
        .unwrap();
    nav_set_default(&node.store, &admin, ws, "ops", 2)
        .await
        .unwrap();

    // Unset-vs-set is covered in nav_test.rs; here the payload SHAPE over the bridge is the point.
    let member = principal("user:ben", ws, &[RESOLVE]);
    let got = call(&node, &member, ws, "nav.get_default", json!({}))
        .await
        .expect("nav.get_default dispatches for a member (alias → nav.resolve)");
    assert_eq!(
        got,
        json!({ "id": "ops" }),
        "the bridge answers the same `{{id}}` envelope the route serves: {got}"
    );

    // The admin reaches it too — the read is not accidentally *exclusive* to non-authors.
    let got = call(&node, &admin, ws, "nav.get_default", json!({}))
        .await
        .expect("nav.get_default dispatches for the author as well");
    assert_eq!(got, json!({ "id": "ops" }));
}

/// The wall still stands over the bridge: a caller holding no nav grant at all is refused — and by
/// SHAPE, so a `NotFound` (a missing dispatch arm, or a typo in the tool name) fails loudly rather
/// than reading as a correct refusal.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn no_nav_grant_is_a_real_denial_not_no_such_tool() {
    let ws = "ws-nav-default-dispatch-deny";
    let node = Arc::new(Node::boot().await.unwrap());
    let nobody = principal("user:mallory", ws, &[]);

    let err = call(&node, &nobody, ws, "nav.get_default", json!({}))
        .await
        .expect_err("a caller with no nav grant is refused");
    assert!(
        matches!(err, ToolError::Denied),
        "expected a REAL denial; a NotFound here means the dispatch arm is missing: {err:?}"
    );
}
