//! Entity version history — the mandatory capability-deny and workspace-isolation categories
//! (`docs/scope/testing/testing-scope.md` §2.1–2.2), plus the undo interaction the scope names.
//!
//! Real infra (rule #9): a booted node, the real MCP bridge, real save verbs. Every denial here is
//! produced by the shipped gate, not by a test double.
//!
//! What is proven here:
//!   - **the named deny** — a caller holding `versions.restore` but NOT the kind's save cap is
//!     refused (the no-escalation check), while the same caller can still LIST;
//!   - `versions.get` is refused without its own grant, even for a caller who may list;
//!   - `versions.config.set` is refused for a non-admin (and readable by a member);
//!   - a denial is OPAQUE — a refused caller cannot tell a real entity from a fictional one;
//!   - **workspace isolation** — ws-A's rings are invisible in ws-B, and a cross-workspace restore
//!     by version id is refused rather than applied;
//!   - the built-in role bundles actually carry these caps (a verb nobody can press is not shipped);
//!   - **undo after a dashboard restore** returns the pre-restore record.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    admin_only_caps, call_tool, history_list, member_role_caps, undo as host_undo,
    viewer_role_caps, Node,
};
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
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

/// An author who can save dashboards AND drive the versions family — the happy-path principal.
fn author(sub: &str, ws: &str) -> Principal {
    principal(
        sub,
        ws,
        &[
            "mcp:dashboard.save:call",
            "mcp:dashboard.get:call",
            "mcp:versions.list:call",
            "mcp:versions.get:call",
            "mcp:versions.restore:call",
            "mcp:versions.config.get:call",
            "mcp:history.list:call",
            "mcp:undo:call",
            "store:*:read",
            "store:*:write",
        ],
    )
}

async fn call(node: &Arc<Node>, p: &Principal, ws: &str, tool: &str, args: Value) -> Value {
    let out = call_tool(node, p, ws, tool, &args.to_string())
        .await
        .unwrap_or_else(|e| panic!("{tool} dispatches: {e}"));
    serde_json::from_str(&out).unwrap_or(Value::String(out))
}

async fn try_call(
    node: &Arc<Node>,
    p: &Principal,
    ws: &str,
    tool: &str,
    args: Value,
) -> Result<Value, String> {
    match call_tool(node, p, ws, tool, &args.to_string()).await {
        Ok(out) => Ok(serde_json::from_str(&out).unwrap_or(Value::String(out))),
        Err(e) => Err(e.to_string()),
    }
}

/// Seed one dashboard with two versions and return `(newest, oldest)` version ids.
async fn seed(node: &Arc<Node>, p: &Principal, ws: &str, id: &str) -> (String, String) {
    call(
        node,
        p,
        ws,
        "dashboard.save",
        json!({ "id": id, "title": "First", "cells": [], "now": 1 }),
    )
    .await;
    call(
        node,
        p,
        ws,
        "dashboard.save",
        json!({ "id": id, "title": "Second", "cells": [], "now": 2 }),
    )
    .await;
    let rows = call(
        node,
        p,
        ws,
        "versions.list",
        json!({ "kind": "dashboard", "id": id }),
    )
    .await;
    let rows = rows["versions"].as_array().expect("rows").clone();
    assert_eq!(rows.len(), 2, "two distinct saves, two versions");
    (
        rows[0]["version_id"].as_str().expect("newest").to_string(),
        rows[1]["version_id"].as_str().expect("oldest").to_string(),
    )
}

// ---------------------------------------------------------------------------------------------
// §2.1 capability-deny
// ---------------------------------------------------------------------------------------------

/// **The named deny from the scope.** A caller holding `versions.list` (and even
/// `versions.restore`) but NOT `mcp:dashboard.save:call` is refused the restore — because restoring
/// IS performing that save. They can still read the history: seeing what you cannot change is
/// correct, not a leak.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn restore_is_refused_without_the_kinds_save_cap() {
    let ws = "ver-deny-escalation";
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let owner = author("user:ada", ws);
    let (_, oldest) = seed(&node, &owner, ws, "ops").await;

    // A viewer-shaped principal: the whole versions read surface AND the restore verb's own cap,
    // but no dashboard authoring. This is precisely the mis-grant the no-escalation check exists
    // for — without it, `versions.restore` would be a back door to `dashboard.save`.
    let viewer = principal(
        "user:vic",
        ws,
        &[
            "mcp:versions.list:call",
            "mcp:versions.get:call",
            "mcp:versions.restore:call",
            "mcp:dashboard.get:call",
            "store:*:read",
        ],
    );

    let rows = call(
        &node,
        &viewer,
        ws,
        "versions.list",
        json!({ "kind": "dashboard", "id": "ops" }),
    )
    .await;
    assert_eq!(
        rows["versions"].as_array().expect("rows").len(),
        2,
        "a viewer READS history — this is what renders the dialog without a Restore button"
    );

    let err = try_call(
        &node,
        &viewer,
        ws,
        "versions.restore",
        json!({ "kind": "dashboard", "id": "ops", "version_id": oldest }),
    )
    .await
    .expect_err("restore without the save cap must be refused");
    assert!(
        err.contains("denied"),
        "the refusal is the standard opaque denial: {err}"
    );

    // Nothing was written: the live record still carries the newest save.
    let live = call(&node, &owner, ws, "dashboard.get", json!({ "id": "ops" })).await;
    assert_eq!(
        live["title"],
        json!("Second"),
        "a refused restore writes nothing"
    );

    // ...and the refusal happens BEFORE the snapshot is loaded. This is what actually pins the
    // no-escalation PRE-check rather than the nested save's own gate: both orderings refuse, but
    // only the pre-check refuses IDENTICALLY for a real version id and a fictional one. Check the
    // snapshot first and the deny path becomes an existence oracle — "denied" means the version is
    // real, "no such tool" means it is not — handing an unauthorized caller a way to enumerate
    // another author's history one guess at a time.
    //
    // (Deleting the pre-check leaves the assertion above green, which is how this test earned its
    // second half: it was revert-checked and survived, so it was not testing what it claimed.)
    let on_fiction = try_call(
        &node,
        &viewer,
        ws,
        "versions.restore",
        json!({ "kind": "dashboard", "id": "ops", "version_id": "01AAAAAAAAAAAAAAAAAAAAAAAA" }),
    )
    .await
    .expect_err("a fictional version is refused too");
    assert_eq!(
        err, on_fiction,
        "a caller without the save cap must not be able to tell a real version from a fictional \
         one — the no-escalation check runs BEFORE the snapshot read"
    );
}

/// `versions.get` needs its OWN grant — a snapshot is the full record content, while the list is
/// provenance. A caller who may list is not thereby entitled to every historical body.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn get_is_refused_without_its_own_grant() {
    let ws = "ver-deny-get";
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let owner = author("user:ada", ws);
    let (newest, _) = seed(&node, &owner, ws, "ops").await;

    let lister = principal("user:len", ws, &["mcp:versions.list:call", "store:*:read"]);
    call(
        &node,
        &lister,
        ws,
        "versions.list",
        json!({ "kind": "dashboard", "id": "ops" }),
    )
    .await;
    let err = try_call(
        &node,
        &lister,
        ws,
        "versions.get",
        json!({ "kind": "dashboard", "id": "ops", "version_id": newest }),
    )
    .await
    .expect_err("get without its grant is refused");
    assert!(err.contains("denied"), "opaque denial: {err}");
}

/// The cap decides how much of EVERY member's history the workspace keeps, so setting it is admin
/// authority. Reading it is not.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn config_set_is_admin_only_while_config_get_is_not() {
    let ws = "ver-deny-config";
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let member = author("user:ada", ws);

    let view = call(&node, &member, ws, "versions.config.get", json!({})).await;
    assert_eq!(view["cap"], json!(20), "a member reads the cap in force");
    assert_eq!(view["default_cap"], json!(20));

    let err = try_call(
        &node,
        &member,
        ws,
        "versions.config.set",
        json!({ "cap": 5 }),
    )
    .await
    .expect_err("a member may not lower the workspace's retention");
    assert!(err.contains("denied"), "opaque denial: {err}");

    // The cap is unchanged — a refused set is not a partial set.
    let view = call(&node, &member, ws, "versions.config.get", json!({})).await;
    assert_eq!(view["cap"], json!(20));
}

/// A denial must carry no existence signal: refusing `versions.get` on a REAL version and on a
/// fictional one must be indistinguishable, or the deny path becomes an entity oracle.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_denial_reveals_nothing_about_what_exists() {
    let ws = "ver-deny-opaque";
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let owner = author("user:ada", ws);
    let (real, _) = seed(&node, &owner, ws, "ops").await;

    let nobody = principal("user:nemo", ws, &["store:*:read"]);
    let on_real = try_call(
        &node,
        &nobody,
        ws,
        "versions.get",
        json!({ "kind": "dashboard", "id": "ops", "version_id": real }),
    )
    .await
    .expect_err("denied");
    let on_fiction = try_call(
        &node,
        &nobody,
        ws,
        "versions.get",
        json!({ "kind": "dashboard", "id": "no-such-board", "version_id": "01NOPE" }),
    )
    .await
    .expect_err("denied");
    assert_eq!(
        on_real, on_fiction,
        "the deny path must not distinguish a real entity from a fictional one"
    );
}

/// The DEGRADATION CONTRACT the downstream shell depends on: its `versionsAvailable()` probe hides
/// every entry point unless `versions.list` appears in the caller's own `tools.catalog`. That
/// catalog is cap-filtered server-side, so a viewer-tier caller must SEE the verb there — otherwise
/// the whole feature silently vanishes for exactly the tier it was designed to serve, and it looks
/// like an old node rather than a cap bug.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_viewer_tier_caller_sees_versions_list_in_its_own_tool_catalog() {
    let ws = "ver-probe";
    let node = Arc::new(Node::boot().await.expect("node boots"));

    // Exactly the viewer bundle's versions caps, plus the catalog read itself.
    let viewer = principal(
        "user:vic",
        ws,
        &[
            "mcp:tools.catalog:call",
            "mcp:versions.list:call",
            "mcp:versions.get:call",
            "mcp:versions.config.get:call",
            "store:*:read",
        ],
    );
    let cat = call(&node, &viewer, ws, "tools.catalog", json!({}))
        .await
        .to_string();
    assert!(
        cat.contains("versions.list"),
        "a viewer must see versions.list in its catalog — the shell's availability probe reads it"
    );
    assert!(
        !cat.contains("versions.restore"),
        "...and must NOT see restore, which it cannot call — the catalog's rule is to advertise a \
         tool only if the call would be allowed"
    );
}

/// The exposure check the undo scope learned the hard way: a verb nobody's role can call is not
/// shipped. The built-in bundles must actually carry these caps at the tiers the scope assigns.
#[test]
fn the_builtin_role_bundles_carry_the_versions_caps() {
    let viewer = viewer_role_caps();
    let member = member_role_caps();
    let admin_only = admin_only_caps();

    for read in [
        "mcp:versions.list:call",
        "mcp:versions.get:call",
        "mcp:versions.config.get:call",
    ] {
        assert!(
            viewer.contains(&read.to_string()),
            "a viewer must reach {read}"
        );
        assert!(
            member.contains(&read.to_string()),
            "a member must reach {read}"
        );
    }
    assert!(
        !viewer.contains(&"mcp:versions.restore:call".to_string()),
        "a viewer must NOT hold restore — history is read-only at that tier"
    );
    assert!(
        member.contains(&"mcp:versions.restore:call".to_string()),
        "a member restores — a verb no role can press is not shipped"
    );
    assert!(
        admin_only.contains(&"mcp:versions.config.set:call".to_string()),
        "the retention cap is workspace administration"
    );
    assert!(
        !member.contains(&"mcp:versions.config.set:call".to_string()),
        "a member must not be able to shrink everyone else's history"
    );
}

// ---------------------------------------------------------------------------------------------
// §2.2 workspace isolation
// ---------------------------------------------------------------------------------------------

/// The hard wall: rings written in ws A are invisible in ws B, and a cross-workspace restore by
/// version id is REFUSED (as `NotFound`), never applied. Both workspaces here use the SAME entity id
/// and the SAME actor, so nothing but the workspace separates them — which is the point.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rings_are_invisible_across_workspaces_and_a_cross_ws_restore_is_refused() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws_a = "ver-iso-a";
    let ws_b = "ver-iso-b";
    let in_a = author("user:ada", ws_a);
    let in_b = author("user:ada", ws_b);

    let (_, a_oldest) = seed(&node, &in_a, ws_a, "shared-id").await;

    // ws B has its own, different history for the SAME id.
    call(
        &node,
        &in_b,
        ws_b,
        "dashboard.save",
        json!({ "id": "shared-id", "title": "B only", "cells": [], "now": 1 }),
    )
    .await;

    let b_rows = call(
        &node,
        &in_b,
        ws_b,
        "versions.list",
        json!({ "kind": "dashboard", "id": "shared-id" }),
    )
    .await;
    assert_eq!(
        b_rows["versions"].as_array().expect("rows").len(),
        1,
        "ws B sees only its own ring — ws A's two versions are invisible"
    );

    // A version id minted in ws A is not resolvable in ws B, and restoring it does nothing there.
    let err = try_call(
        &node,
        &in_b,
        ws_b,
        "versions.restore",
        json!({ "kind": "dashboard", "id": "shared-id", "version_id": a_oldest }),
    )
    .await
    .expect_err("a cross-workspace version id must not resolve");
    assert!(err.contains("no such tool"), "an opaque NotFound: {err}");

    let live_b = call(
        &node,
        &in_b,
        ws_b,
        "dashboard.get",
        json!({ "id": "shared-id" }),
    )
    .await;
    assert_eq!(
        live_b["title"],
        json!("B only"),
        "ws B's record was not overwritten with ws A's content"
    );
    let live_a = call(
        &node,
        &in_a,
        ws_a,
        "dashboard.get",
        json!({ "id": "shared-id" }),
    )
    .await;
    assert_eq!(live_a["title"], json!("Second"), "ws A is untouched too");
}

// ---------------------------------------------------------------------------------------------
// The undo interaction
// ---------------------------------------------------------------------------------------------

/// The scope's compose case: a restore is itself an undoable step. Because the restore's own save
/// runs at depth+1 (below the capture chokepoint), it is the `versions.restore` CALL that
/// `undo_capture` journals — against the dashboard record it rewrites, via the versions kind plan
/// table. Ctrl+Z after a restore must therefore return the PRE-restore record.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn undo_after_a_dashboard_restore_returns_the_pre_restore_record() {
    let ws = "ver-undo";
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = author("user:ada", ws);
    let (_, oldest) = seed(&node, &p, ws, "ops").await;

    call(
        &node,
        &p,
        ws,
        "versions.restore",
        json!({ "kind": "dashboard", "id": "ops", "version_id": oldest, "now": 3 }),
    )
    .await;
    let live = call(&node, &p, ws, "dashboard.get", json!({ "id": "ops" })).await;
    assert_eq!(live["title"], json!("First"), "the restore landed");

    // The restore is journaled on the dashboard's OWN undo surface (its record id), as an undoable
    // step — not as an opaque non-generic marker.
    let items = history_list(&node.store, &p, ws, "user:ada", "ops")
        .await
        .expect("history reads")
        .items;
    assert_eq!(
        items[0].tool, "versions.restore",
        "the newest step is the restore itself, not the nested save"
    );
    assert!(items[0].undoable, "a restore is undoable");

    host_undo(&node.store, &p, ws, "user:ada", "ops")
        .await
        .expect("undo applies");
    let live = call(&node, &p, ws, "dashboard.get", json!({ "id": "ops" })).await;
    assert_eq!(
        live["title"],
        json!("Second"),
        "undo returns the record to what it was BEFORE the restore"
    );
}
