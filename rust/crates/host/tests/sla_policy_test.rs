//! The **SLA policy plane** — `policy.sla.set` / `policy.sla.list` — over a REAL booted `Node`
//! (`docs/scope/insights/case-plane-scope.md`, the `service_policy` row). Real store (`mem://`),
//! real caps, the real `call_tool` MCP bridge. NO mocks (CLAUDE §4): every policy is written
//! through the verb under test and read back through the other one.
//!
//! Mandatory categories, per verb: **capability-deny** (including the property only the OUTER gate
//! has — a denied caller cannot tell a real id from a fictional one) and **workspace isolation**.
//!
//! Beyond the mandatory two: the list's *order* is asserted, because that order IS the resolution
//! ladder the sla-clock reactor applies — an admin who cannot read the precedence off the page has
//! to read the code instead.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, member_role_caps, viewer_role_caps, workspace_admin_role_caps, Node};
use lb_mcp::ToolError;
use serde_json::{json, Value};

const SET: &str = "mcp:policy.sla.set:call";
const LIST: &str = "mcp:policy.sla.list:call";

/// The admin token most cases use.
const ADMIN: &[&str] = &[SET, LIST];

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

/// A policy payload. `match` axes are opaque strings supplied as data — rule 10: no verb, no cap
/// and no test here knows what a category *value* means.
fn policy(id: &str, m: Value) -> Value {
    json!({
        "id": id,
        "name": id,
        "match": m,
        "respond_h": 4,
        "resolve_h": 24,
    })
}

/// The ids `policy.sla.list` returned, in the order it returned them.
fn ids(out: &Value) -> Vec<String> {
    out.as_array()
        .or_else(|| out.get("policies").and_then(Value::as_array))
        .expect("a list of policies")
        .iter()
        .map(|p| p["id"].as_str().expect("an id").to_string())
        .collect()
}

async fn seed(node: &Arc<Node>, p: &Principal, ws: &str, id: &str, m: Value) {
    call(node, p, ws, "policy.sla.set", policy(id, m))
        .await
        .unwrap_or_else(|e| panic!("set {id} in {ws}: {e:?}"));
}

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

// --- the ladder, the upsert, the rejects ---------------------------------------------------------

/// The list order IS the resolution ladder: three pinned axes, then two, then one, then the
/// workspace default; equally specific rows by id ascending. An admin reads precedence off the page.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_list_is_ordered_most_specific_first() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let admin = principal("user:admin", "nube", ADMIN);

    // Seeded in a deliberately unhelpful order.
    seed(&node, &admin, "nube", "default", json!({})).await;
    seed(&node, &admin, "nube", "zeta-site", json!({ "site": "s1" })).await;
    seed(&node, &admin, "nube", "alpha-site", json!({ "site": "s2" })).await;
    seed(
        &node,
        &admin,
        "nube",
        "all-three",
        json!({ "site": "s1", "category": "c1", "severity": "critical" }),
    )
    .await;
    seed(
        &node,
        &admin,
        "nube",
        "two",
        json!({ "site": "s1", "category": "c1" }),
    )
    .await;

    let out = call(&node, &admin, "nube", "policy.sla.list", json!({}))
        .await
        .expect("lists");
    assert_eq!(
        ids(&out),
        ["all-three", "two", "alpha-site", "zeta-site", "default"],
        "most specific first; ties by id ascending"
    );
}

/// `set` is an upsert keyed by id: a second write replaces the row rather than adding a second one.
/// This is what lets the settings surface edit in place and a pack re-seed idempotently.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn set_upserts_by_id_and_defaults_the_hold_down_window() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let admin = principal("user:admin", "nube", ADMIN);

    seed(&node, &admin, "nube", "contract", json!({})).await;
    call(
        &node,
        &admin,
        "nube",
        "policy.sla.set",
        json!({ "id": "contract", "name": "renegotiated", "match": {}, "respond_h": 2, "resolve_h": 8 }),
    )
    .await
    .expect("second write");

    let out = call(&node, &admin, "nube", "policy.sla.list", json!({}))
        .await
        .expect("lists");
    assert_eq!(ids(&out), ["contract"], "one row, not two");
    let row = &out
        .as_array()
        .or_else(|| out.get("policies").and_then(Value::as_array))
        .unwrap()[0];
    assert_eq!(row["name"], "renegotiated");
    assert_eq!(row["respond_h"], 2);
    // The scope's decision, surviving the round trip: a policy that says nothing about hold-down
    // holds down for 14 days.
    assert_eq!(row["hold_down_days"], 14);
}

/// A policy that cannot govern work is refused at the door, not stored and discovered later by a
/// reactor computing a nonsense deadline.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_unusable_policy_is_refused_and_writes_nothing() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let admin = principal("user:admin", "nube", ADMIN);

    // Due to be FIXED before anyone is due to have LOOKED at it.
    assert!(call(
        &node,
        &admin,
        "nube",
        "policy.sla.set",
        json!({ "id": "backwards", "match": {}, "respond_h": 24, "resolve_h": 4 }),
    )
    .await
    .is_err());

    // A timezone no tzdata knows.
    assert!(call(
        &node,
        &admin,
        "nube",
        "policy.sla.set",
        json!({
            "id": "bad-tz", "match": {}, "respond_h": 1, "resolve_h": 2,
            "calendar": { "kind": "business", "tz": "Mars/Olympus" },
        }),
    )
    .await
    .is_err());

    let out = call(&node, &admin, "nube", "policy.sla.list", json!({}))
        .await
        .expect("lists");
    assert!(ids(&out).is_empty(), "neither reject was stored");
}

/// A real business calendar round-trips through the store intact — the hours, the Monday-first
/// weekly pattern and the holiday list all survive, because the deadline arithmetic downstream is
/// only as good as what was persisted.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_business_calendar_round_trips() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let admin = principal("user:admin", "nube", ADMIN);

    let office = json!({
        "kind": "business",
        "tz": "Australia/Brisbane",
        "hours": [
            { "open_min": 540, "close_min": 1020 },
            { "open_min": 540, "close_min": 1020 },
            { "open_min": 540, "close_min": 1020 },
            { "open_min": 540, "close_min": 1020 },
            { "open_min": 540, "close_min": 1020 },
            { "open_min": 0, "close_min": 0 },
            { "open_min": 0, "close_min": 0 },
        ],
        "holidays": ["2026-12-25"],
    });
    call(
        &node,
        &admin,
        "nube",
        "policy.sla.set",
        json!({ "id": "office", "match": {}, "respond_h": 2, "resolve_h": 20, "calendar": office }),
    )
    .await
    .expect("set");

    let out = call(&node, &admin, "nube", "policy.sla.list", json!({}))
        .await
        .expect("lists");
    let row = &out
        .as_array()
        .or_else(|| out.get("policies").and_then(Value::as_array))
        .unwrap()[0];
    assert_eq!(row["calendar"]["tz"], "Australia/Brisbane");
    assert_eq!(row["calendar"]["kind"], "business");
    // Index 0 is MONDAY and index 5/6 the weekend — the convention every deadline depends on.
    assert_eq!(row["calendar"]["hours"][0]["open_min"], 540);
    assert_eq!(row["calendar"]["hours"][5]["close_min"], 0);
    assert_eq!(row["calendar"]["holidays"][0], "2026-12-25");
}

// --- MANDATORY: the POSITIVE gate test ------------------------------------------------------------

/// **A shipped-but-unusable verb is the failure mode this test exists for.** A capability that
/// exists in NO role bundle makes its verb `Denied` for every caller including admins, and every
/// negative test above still passes — they assert a deny, and a deny is exactly what a missing cap
/// produces. `media.upload_*` and `series.retention.delete` both shipped in that state and were
/// found on a live node.
///
/// So: mint a token carrying the REAL `workspace-admin` bundle (not a hand-written cap list) and
/// prove both verbs actually run. If either cap ever falls out of `ADMIN_ONLY_CAPS`, this goes red.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_real_workspace_admin_bundle_can_reach_both_verbs() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let bundle = workspace_admin_role_caps();
    assert!(
        bundle.iter().any(|c| c == SET) && bundle.iter().any(|c| c == LIST),
        "both caps must exist in the shipped workspace-admin bundle, or the verbs are \
         unreachable for EVERY caller: {bundle:?}"
    );

    let caps: Vec<&str> = bundle.iter().map(String::as_str).collect();
    let admin = principal("user:admin", "nube", &caps);
    call(
        &node,
        &admin,
        "nube",
        "policy.sla.set",
        policy("default", json!({})),
    )
    .await
    .expect("a real admin bundle can SET");
    let out = call(&node, &admin, "nube", "policy.sla.list", json!({}))
        .await
        .expect("a real admin bundle can LIST");
    assert_eq!(ids(&out), ["default"]);
}

/// The other half of the tiering: neither cap leaks into the member or viewer bundle, and no
/// wildcard in those bundles spans them. Asserted against the REAL bundles, so a future edit that
/// widens `member` cannot quietly hand the commercial terms to everyone.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn neither_cap_is_in_the_member_or_viewer_bundle() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    for (tier, bundle) in [
        ("member", member_role_caps()),
        ("viewer", viewer_role_caps()),
    ] {
        for cap in [SET, LIST] {
            assert!(
                !bundle.iter().any(|c| c == cap),
                "{cap} must not be in the {tier} bundle"
            );
        }
        // And the bundle as a WHOLE — wildcards included — cannot reach either verb.
        let caps: Vec<&str> = bundle.iter().map(String::as_str).collect();
        let p = principal("user:tiered", "nube", &caps);
        for tool in ["policy.sla.set", "policy.sla.list"] {
            assert!(
                matches!(
                    call(&node, &p, "nube", tool, policy("x", json!({}))).await,
                    Err(ToolError::Denied)
                ),
                "the full {tier} bundle must not reach {tool} (check for a spanning wildcard)"
            );
        }
    }
}

/// **Dispatchable but invisible is its own kind of broken.** The console and the agent's menu are
/// both built from `tools.catalog`, so a verb missing from the host catalog can be called only by
/// someone who already knows its name — which is how whole families (`datasource.`, `viz.`,
/// `flows.`) once went missing. An admin must SEE both verbs.
///
/// The catalog is gated by the same `gate_tool_for` the dispatcher uses, so this also re-proves the
/// cardinal rule from the other side: advertise a tool only if the call would allow it.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_admin_sees_both_verbs_in_the_tools_catalog() {
    let node = Node::boot().await.expect("node boots");
    let mut caps: Vec<String> = workspace_admin_role_caps();
    caps.push("mcp:tools.catalog:call".into());
    let refs: Vec<&str> = caps.iter().map(String::as_str).collect();
    let admin = principal("user:admin", "nube", &refs);

    let catalog = lb_host::tools_catalog(&node, &admin, "nube")
        .await
        .expect("catalog");
    for verb in ["policy.sla.set", "policy.sla.list"] {
        assert!(
            catalog.tools.iter().any(|d| d.name == verb),
            "{verb} is dispatchable but absent from tools.catalog — invisible to the console \
             and the agent menu"
        );
    }
}
