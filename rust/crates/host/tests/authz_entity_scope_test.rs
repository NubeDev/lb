//! `authz.entity_scope` — the menu-aware reach question, answered by the resolver lb enforces with.
//!
//! The setup is the one `entity_scoped_data_test.rs` uses for federated reads (a team handed a menu
//! that marks one site), minus the federation sidecar: this verb reads no datasource. Real embedded
//! store, real caps, real nav share — no mocks. Covers:
//!   - a member's OWN scope is the sites on their handed menu; an admin's is `all`;
//!   - delegated reach — allow: a caller holding `mcp:authz.delegate_reach:call` gets the SUBJECT's
//!     scope (the native-sidecar-for-a-viewer flow), including "nothing" for a user with no menu;
//!   - delegated reach — deny: a `subject` without the delegation cap is a 403, never a fallback;
//!   - capability deny: no `mcp:authz.entity_scope:call` → 403;
//!   - freshness: a membership change reaches the answer at once;
//!   - workspace isolation: the same subject asked from another workspace reaches nothing.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, Node};
use lb_mcp::ToolError;
use serde_json::{json, Value};

const WS: &str = "estate";
const OTHER_WS: &str = "elsewhere";
const SCOPE: &str = "mcp:authz.entity_scope:call";
const DELEGATE: &str = "mcp:authz.delegate_reach:call";

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
    verify(&key, &mint(&key, &claims), 1).unwrap()
}

/// A workspace admin: `teams.manage` is an admin-marker cap, which is what makes it unrestricted.
fn admin() -> Principal {
    principal(
        "user:admin",
        WS,
        &[
            SCOPE,
            "mcp:nav.save:call",
            "mcp:nav.share:call",
            "mcp:teams.manage:call",
            "mcp:teams.create:call",
            "mcp:members.add:call",
        ],
    )
}

/// What an extension's sidecar token carries for this flow: the verb and the delegation marker.
fn sidecar(ws: &str) -> Principal {
    principal("ext:ros", ws, &[SCOPE, DELEGATE])
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

/// Team g1 holds user:m1, and a menu shared with g1 marks ONLY "Site A" (nested, as in the
/// federated-read test, to prove depth does not matter).
async fn setup() -> Arc<Node> {
    let node = Arc::new(Node::boot().await.unwrap());
    let admin = admin();
    for (tool, input) in [
        ("teams.create", json!({"team": "g1", "name": "Group 1"})),
        ("members.add", json!({"team": "g1", "user": "user:m1"})),
        (
            "nav.save",
            json!({"id": "g1menu", "title": "Group 1", "now": 1, "items": [
                {"kind": "group", "label": "VIC", "items": [
                    {"kind": "group", "label": "Site A", "entity": {"table": "site", "id": "Site A"}, "items": []}
                ]}
            ]}),
        ),
        (
            "nav.share",
            json!({"id": "g1menu", "visibility": "team", "team": "g1", "now": 2}),
        ),
    ] {
        call(&node, &admin, WS, tool, input).await.unwrap();
    }
    node
}

fn ids(v: &Value) -> Vec<String> {
    v["filter"]["ids"]
        .as_array()
        .unwrap_or_else(|| panic!("expected an id list, got {v}"))
        .iter()
        .map(|i| i.as_str().unwrap().to_string())
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_member_reaches_their_menu_sites_and_an_admin_reaches_all() {
    let node = setup().await;
    let m1 = principal("user:m1", WS, &[SCOPE]);
    let own = call(
        &node,
        &m1,
        WS,
        "authz.entity_scope",
        json!({"table": "site"}),
    )
    .await
    .unwrap();
    assert_eq!(ids(&own), ["Site A"]);

    let all = call(
        &node,
        &admin(),
        WS,
        "authz.entity_scope",
        json!({"table": "site"}),
    )
    .await
    .unwrap();
    assert_eq!(all, json!({"filter": "all"}));

    // Another table is a different question: the menu marks no `building`.
    let other = call(
        &node,
        &m1,
        WS,
        "authz.entity_scope",
        json!({"table": "building"}),
    )
    .await
    .unwrap();
    assert!(ids(&other).is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_delegating_caller_gets_the_subjects_scope_not_its_own() {
    let node = setup().await;
    let ext = sidecar(WS);
    let m1 = call(
        &node,
        &ext,
        WS,
        "authz.entity_scope",
        json!({"table": "site", "subject": "user:m1"}),
    )
    .await
    .unwrap();
    assert_eq!(ids(&m1), ["Site A"]);

    // A user handed no menu reaches NOTHING — an empty list, never "all".
    let m2 = call(
        &node,
        &ext,
        WS,
        "authz.entity_scope",
        json!({"table": "site", "subject": "user:m2"}),
    )
    .await
    .unwrap();
    assert_eq!(m2, json!({"filter": {"ids": []}}));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_subject_without_the_delegation_cap_is_denied_not_answered_for_the_caller() {
    let node = setup().await;
    // m1 may ask about itself, but naming ANOTHER subject needs the marker cap.
    let m1 = principal("user:m1", WS, &[SCOPE]);
    let err = call(
        &node,
        &m1,
        WS,
        "authz.entity_scope",
        json!({"table": "site", "subject": "user:admin"}),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::Denied), "{err:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_verb_is_denied_without_its_cap() {
    let node = setup().await;
    let bare = principal("user:m1", WS, &["mcp:nav.resolve:call"]);
    let err = call(
        &node,
        &bare,
        WS,
        "authz.entity_scope",
        json!({"table": "site"}),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::Denied), "{err:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bad_arguments_are_refused() {
    let node = setup().await;
    let ext = sidecar(WS);
    for input in [
        json!({"table": "site", "subject": "team:g1"}),
        json!({"table": "site", "subject": "user:"}),
        json!({"table": "site", "sources": ["nav", "tags"]}),
        json!({"subject": "user:m1"}),
    ] {
        let err = call(&node, &ext, WS, "authz.entity_scope", input.clone())
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::BadInput(_)), "{input}: {err:?}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_membership_change_reaches_the_answer_at_once() {
    let node = setup().await;
    let admin = admin();
    let ext = sidecar(WS);
    let ask = || json!({"table": "site", "subject": "user:m1"});
    assert_eq!(
        ids(&call(&node, &ext, WS, "authz.entity_scope", ask())
            .await
            .unwrap()),
        ["Site A"]
    );

    for (tool, input) in [
        ("teams.create", json!({"team": "g2", "name": "Group 2"})),
        (
            "nav.save",
            json!({"id": "g2menu", "title": "Group 2", "now": 3, "items": [
                {"kind": "group", "label": "Site B", "entity": {"table": "site", "id": "Site B"}, "items": []}
            ]}),
        ),
        (
            "nav.share",
            json!({"id": "g2menu", "visibility": "team", "team": "g2", "now": 4}),
        ),
        ("members.add", json!({"team": "g2", "user": "user:m1"})),
    ] {
        call(&node, &admin, WS, tool, input).await.unwrap();
    }
    // The scope was cached by the first ask; the membership write must drop it.
    assert_eq!(
        ids(&call(&node, &ext, WS, "authz.entity_scope", ask())
            .await
            .unwrap()),
        ["Site A", "Site B"]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_subject_resolves_only_inside_the_callers_workspace() {
    let node = setup().await;
    // The same subject, asked from another workspace, holds no menu there.
    let out = call(
        &node,
        &sidecar(OTHER_WS),
        OTHER_WS,
        "authz.entity_scope",
        json!({"table": "site", "subject": "user:m1"}),
    )
    .await
    .unwrap();
    assert_eq!(out, json!({"filter": {"ids": []}}));
}
