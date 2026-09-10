//! The **`tags.*` dispatcher door** — the four tag verbs are reachable over MCP, and the caps they
//! gate on exist in a shipped role bundle (`docs/scope/insights/case-plane-scope.md` §"Verbs",
//! resolved decisions 1–2).
//!
//! ## Why this file exists
//!
//! `tags.add` / `tags.remove` / `tags.of` / `tags.find` have had a full host service
//! (`host/src/tags/`, `call_tags_tool`) and per-verb caps since the tags scope — and **no entry in
//! the dispatcher's host-native table**, so nothing could reach any of them over MCP. Every caller
//! got `no such tool`. That makes the whole tag-precedence rule unreachable in production: there is
//! no wire door through which a human can write a `Human`-sourced edge at all.
//!
//! Two of the four are worse than unreachable — they are *shipped but unusable*, the exact shape
//! `tool_gate.rs` documents four times over: `mcp:tags.remove:call` exists in **no role bundle**, so
//! adding the dispatcher entry alone would leave `tags.remove` `Denied` for every caller including
//! admins. And a missing alias/cap is `Denied`, never `NotFound` — which is why every assertion
//! here is **positive**: only a test that says "this call must SUCCEED" catches it.
//!
//! ## This file is RED until the lead lands the registration
//!
//! The four dispatcher entries, the `tags.of → tags.find` alias and the `AUTHOR_CAPS` addition live
//! in `tool_call.rs` / `tool_gate.rs` / `authz/builtin_roles.rs`, which this agent does not own. Red
//! here means "the door is not registered yet", which is the correct signal, not a broken test.
//!
//! Real booted `Node`: real store (`mem://`), real tag graph, real caps, the real `call_tool`
//! bridge. No mocks (CLAUDE §9). Note the deliberate absence of a `gate_tool_for` unit assertion:
//! that fn and `is_host_native` are `pub(crate)`, so an integration test cannot name them — and
//! naming them would prove less anyway. What matters is not that the table has a row; it is that a
//! caller holding the read cap can make the call, which is what these assertions state.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{author_caps, call_tool, viewer_role_caps, Node};
use lb_mcp::ToolError;
use serde_json::{json, Value};

const ADD: &str = "mcp:tags.add:call";
const REMOVE: &str = "mcp:tags.remove:call";
const FIND: &str = "mcp:tags.find:call";

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

// --- the door itself ---------------------------------------------------------------------------

/// **THE REGRESSION.** All four verbs must dispatch. A verb missing from the host-native table
/// answers `NotFound` ("no such tool") no matter what caps the caller holds, which is why this
/// asserts on the error KIND and not merely on failure.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn all_four_tag_verbs_dispatch_over_mcp() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    let p = principal("user:test", ws, &[ADD, REMOVE, FIND]);

    // A real write, then the three reads/deletes over it — one round trip through the door.
    let calls: Vec<(&str, Value)> = vec![
        (
            "tags.add",
            json!({ "entity": "insight:abc", "key": "classification", "value": "mechanical", "source": "human", "at": 10 }),
        ),
        ("tags.of", json!({ "entity": "insight:abc" })),
        (
            "tags.find",
            json!({ "facets": [{ "key": "classification", "value": "mechanical" }] }),
        ),
        (
            "tags.remove",
            json!({ "entity": "insight:abc", "key": "classification" }),
        ),
    ];

    for (tool, input) in calls {
        let out = call(&node, &p, ws, tool, input).await;
        match out {
            Ok(_) => {}
            Err(ToolError::NotFound) => panic!(
                "{tool} is not in HOST_NATIVE_EXACT — the dispatcher door is unregistered, so no \
                 caller can reach it however many caps they hold"
            ),
            Err(ToolError::Denied) => panic!(
                "{tool} dispatched but was DENIED with its own cap held — the gate is asking for a \
                 cap this caller does not have (a missing/wrong tool_gate alias)"
            ),
            Err(e) => panic!("{tool} failed unexpectedly: {e:?}"),
        }
    }
}

/// **The alias.** Reading ONE entity's tags is `tags.find` narrowed to one entity, not a second
/// privilege — so `tags.of` gates on `mcp:tags.find:call`. Asserted the only way that matters: a
/// caller holding the find cap and **not** an `of` cap must be able to make the call.
///
/// Without the alias this is `Denied`, not `NotFound` — invisible to any test that only checks the
/// happy path with a full-cap principal.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn tags_of_rides_the_find_read_cap() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    let reader = principal("user:viewer", ws, &[FIND]);

    let out = call(&node, &reader, ws, "tags.of", json!({ "entity": "insight:abc" })).await;
    assert!(
        out.is_ok(),
        "tags.of must gate on mcp:tags.find:call (it is tags.find narrowed to one entity): {out:?}"
    );
}

/// A caller holding only the READ cap must not be able to WRITE through the door. The door widens
/// reachability, never authority — the deny half of the same registration.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_read_cap_does_not_carry_the_writes() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    let reader = principal("user:viewer", ws, &[FIND]);

    for (tool, input) in [
        (
            "tags.add",
            json!({ "entity": "insight:abc", "key": "k", "value": "v" }),
        ),
        ("tags.remove", json!({ "entity": "insight:abc", "key": "k" })),
    ] {
        assert!(
            matches!(call(&node, &reader, ws, tool, input).await, Err(ToolError::Denied)),
            "{tool} must be Denied for a read-only caller"
        );
    }
}

// --- the caps the door gates on must exist in a shipped bundle -----------------------------------

/// **THE SHIPPED-BUT-UNUSABLE TRAP.** `mcp:tags.remove:call` is in NO role bundle today, so the
/// dispatcher entry alone leaves the verb `Denied` for every caller including admins. It is an
/// author write, and it belongs beside the `mcp:tags.add:call` that already ships in `AUTHOR_CAPS`:
/// a member who may assert a tag on their own series may also retract one.
#[test]
fn tags_remove_ships_in_the_author_bundle_beside_tags_add() {
    let author = author_caps();
    assert!(
        author.iter().any(|c| c == ADD),
        "the precondition this test is anchored to changed: mcp:tags.add:call left AUTHOR_CAPS"
    );
    assert!(
        author.iter().any(|c| c == REMOVE),
        "mcp:tags.remove:call must join AUTHOR_CAPS beside mcp:tags.add:call — otherwise the verb \
         ships reachable and Denied for everyone, admins included"
    );
}

/// `tags.of` needs no cap of its own precisely because it rides the viewer's `tags.find` — which
/// must therefore actually be in the viewer bundle. Stated so that "we aliased it" and "the target
/// cap exists" can never drift apart.
#[test]
fn the_alias_target_is_a_cap_a_viewer_actually_holds() {
    assert!(
        viewer_role_caps().iter().any(|c| c == FIND),
        "tags.of is aliased onto mcp:tags.find:call, so a viewer must hold it"
    );
    assert!(
        !viewer_role_caps().iter().any(|c| c == "mcp:tags.of:call"),
        "no literal mcp:tags.of:call cap is minted — the alias is the whole mechanism, and a \
         second name for one privilege is how a catalog stops being readable"
    );
}
