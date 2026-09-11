//! The two mandatory walls: a capability deny per verb, and workspace isolation over the policy list.
//!
//! Part of the `sla_policy` suite (see `sla_policy_support.rs` for the fixtures and the
//! full preamble). One binary: `case_suite.rs`.

use super::sla_policy_support::*;

// --- MANDATORY: capability deny ----------------------------------------------------------------

/// Both verbs are ADMIN. A member token — even one holding every *case* cap there is — moves no
/// deadline and reads no contract terms. This is the deny the ADMIN tiering exists to create: the
/// power to edit a policy is the power to reorder every case in the workspace.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_member_grant_buys_no_policy_power() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let admin = principal("user:admin", "nube", ADMIN);
    seed(&node, &admin, "nube", "default", json!({})).await;

    // A token with no policy caps at all.
    let member = principal("user:bob", "nube", &["mcp:case.list:call"]);
    assert!(
        matches!(
            call(
                &node,
                &member,
                "nube",
                "policy.sla.set",
                policy("default", json!({}))
            )
            .await,
            Err(ToolError::Denied)
        ),
        "a member must not write a policy"
    );
    assert!(
        matches!(
            call(&node, &member, "nube", "policy.sla.list", json!({})).await,
            Err(ToolError::Denied)
        ),
        "a member must not read the contract terms"
    );

    // And the deny happened before any write — the seeded row is untouched and still alone.
    let out = call(&node, &admin, "nube", "policy.sla.list", json!({}))
        .await
        .expect("admin lists");
    assert_eq!(ids(&out), ["default"]);
}

/// The two caps are independent: holding the read does not buy the write. If `list` alone let a
/// caller `set`, the ADMIN tiering would be decorative.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_read_cap_does_not_buy_the_write() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let reader = principal("user:read", "nube", &[LIST]);
    assert!(
        matches!(
            call(
                &node,
                &reader,
                "nube",
                "policy.sla.set",
                policy("p", json!({}))
            )
            .await,
            Err(ToolError::Denied)
        ),
        "policy.sla.list must not buy policy.sla.set"
    );
    let writer = principal("user:write", "nube", &[SET]);
    assert!(
        matches!(
            call(&node, &writer, "nube", "policy.sla.list", json!({})).await,
            Err(ToolError::Denied)
        ),
        "policy.sla.set must not buy policy.sla.list"
    );
    // The reader really can read — proving the deny above is about the cap, not a broken harness.
    assert!(call(&node, &reader, "nube", "policy.sla.list", json!({}))
        .await
        .is_ok());
}

/// The property only the OUTER gate has: a denied caller cannot distinguish a policy id that EXISTS
/// from one that does not. If these two errors ever differ, the deny has moved inside the verb and
/// become an existence oracle over the workspace's contracts.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn deny_is_identical_for_a_real_id_and_a_fictional_one() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let admin = principal("user:admin", "nube", ADMIN);
    seed(&node, &admin, "nube", "real-contract", json!({})).await;

    let member = principal("user:bob", "nube", &[]);
    let on_real = call(
        &node,
        &member,
        "nube",
        "policy.sla.set",
        policy("real-contract", json!({})),
    )
    .await;
    let on_fake = call(
        &node,
        &member,
        "nube",
        "policy.sla.set",
        policy("no-such-policy", json!({})),
    )
    .await;
    let redact = |r: &Result<Value, ToolError>, id: &str| format!("{r:?}").replace(id, "<ID>");
    assert_eq!(
        redact(&on_real, "real-contract"),
        redact(&on_fake, "no-such-policy"),
        "a real policy id must deny identically to a fictional one"
    );
    assert!(matches!(on_real, Err(ToolError::Denied)));
}

// --- MANDATORY: workspace isolation -------------------------------------------------------------

/// ws-B cannot read ws-A's policies and cannot overwrite one by id. The id collision is the sharp
/// case: both workspaces hold a policy called `default`, and each must see only its own.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_workspace_sees_only_its_own_policies() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let a = principal("user:test", "ws-a", ADMIN);
    let b = principal("user:test", "ws-b", ADMIN);

    seed(&node, &a, "ws-a", "default", json!({})).await;
    seed(&node, &a, "ws-a", "ws-a-only", json!({ "site": "a-site" })).await;
    seed(&node, &b, "ws-b", "default", json!({})).await;

    // ws-B's list holds its own `default` and NOTHING of ws-A's — not even the row whose id it
    // shares.
    let b_list = call(&node, &b, "ws-b", "policy.sla.list", json!({}))
        .await
        .expect("ws-b lists");
    assert_eq!(ids(&b_list), ["default"], "ws-B sees only its own rows");

    // ws-B writing to the SHARED id changes only ws-B's row.
    call(
        &node,
        &b,
        "ws-b",
        "policy.sla.set",
        json!({ "id": "default", "name": "ws-b rewrite", "match": {}, "respond_h": 1, "resolve_h": 2 }),
    )
    .await
    .expect("ws-b rewrites its own default");

    let a_list = call(&node, &a, "ws-a", "policy.sla.list", json!({}))
        .await
        .expect("ws-a lists");
    let a_default = a_list
        .as_array()
        .or_else(|| a_list.get("policies").and_then(Value::as_array))
        .unwrap()
        .iter()
        .find(|p| p["id"] == "default")
        .expect("ws-a still has its default");
    assert_eq!(a_default["respond_h"], 4, "ws-A's row was not overwritten");
    assert_eq!(a_default["name"], "default");
    assert_eq!(ids(&a_list).len(), 2, "and ws-A still has both of its rows");

    // A ws-A principal cannot reach ws-B's namespace by naming it.
    assert!(
        call(&node, &a, "ws-b", "policy.sla.list", json!({}))
            .await
            .is_err(),
        "a ws-A token must not list ws-B"
    );
}
