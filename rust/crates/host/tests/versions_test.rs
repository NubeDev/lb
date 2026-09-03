//! Entity version history — the ring, dedupe, and restore, end to end
//! (`docs/scope/versions/entity-version-history-scope.md`, `NubeDev/lb#112`).
//!
//! Real infra throughout (rule #9): a booted `Node` with the in-memory store, dispatched through the
//! actual `call_tool` MCP bridge, with real `dashboard.save` / `flows.save` / `rules.save` verbs.
//! Nothing here writes an `entity_version` row by hand — every row in every assertion was produced
//! by the depth-0 capture seam, which is the only way these tests can prove capture happens at all.
//!
//! What is proven here (the scope's acceptance list, minus authz/isolation which live in
//! `versions_authz_test.rs`):
//!   - 25 saves leave exactly 20 ring rows, newest-first, oldest trimmed;
//!   - concurrent saves of ONE entity never over-grow the ring;
//!   - a no-op save is deduped (it does not burn a slot);
//!   - restore round-trips per kind — dashboard, flow, and rule content match the snapshot;
//!   - restore re-runs the kind's validators (an invalid old snapshot is REFUSED, not written);
//!   - restore works after delete (it re-creates the entity);
//!   - a restore appends a new head (history stays append-only and re-restorable);
//!   - the per-kind / per-workspace cap override is honoured by capture;
//!   - `versions.*` is in `system.tools` with validating args.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, Node};
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

/// Every cap the three save verbs + the versions family touch. `store:*:write` / `store:*:read` are
/// what `flows.save` / `rules.save` gate their upserts on.
const FULL: &[&str] = &[
    "mcp:dashboard.save:call",
    "mcp:dashboard.get:call",
    "mcp:dashboard.delete:call",
    "mcp:flows.save:call",
    "mcp:flows.get:call",
    "mcp:rules.save:call",
    "mcp:rules.get:call",
    "mcp:rules.delete:call",
    "mcp:versions.list:call",
    "mcp:versions.get:call",
    "mcp:versions.restore:call",
    "mcp:versions.config.get:call",
    "mcp:versions.config.set:call",
    "mcp:system.tools:call",
    "mcp:tools.catalog:call",
    "store:*:read",
    "store:*:write",
];

fn member(ws: &str) -> Principal {
    principal("user:test", ws, FULL)
}

/// Dispatch through the real MCP bridge and decode the JSON result.
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

async fn list(node: &Arc<Node>, p: &Principal, ws: &str, kind: &str, id: &str) -> Vec<Value> {
    let out = call(
        node,
        p,
        ws,
        "versions.list",
        json!({ "kind": kind, "id": id }),
    )
    .await;
    out["versions"].as_array().cloned().unwrap_or_default()
}

fn save_dashboard(id: &str, title: &str, now: u64) -> Value {
    json!({ "id": id, "title": title, "cells": [], "now": now })
}

// ---------------------------------------------------------------------------------------------
// The ring
// ---------------------------------------------------------------------------------------------

/// The scope's headline retention property: past the cap, the OLDEST row is trimmed, in the same
/// transaction as the newest insert — so 25 saves leave exactly 20 rows, newest-first, and the
/// survivors are saves 6..=25.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn twenty_five_saves_leave_exactly_twenty_ring_rows_newest_first() {
    let ws = "ver-ring";
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = member(ws);

    for i in 1..=25u64 {
        call(
            &node,
            &p,
            ws,
            "dashboard.save",
            save_dashboard("ops", &format!("Ops {i}"), i),
        )
        .await;
    }

    let rows = list(&node, &p, ws, "dashboard", "ops").await;
    assert_eq!(rows.len(), 20, "the ring caps at DEFAULT_VERSION_CAP");
    assert_eq!(
        rows[0]["snapshot"],
        Value::Null,
        "versions.list is metadata-only — it must never ship snapshots"
    );

    // Newest-first, and the survivors are the LAST 20 saves (1..=5 were trimmed). We read the
    // titles back through versions.get, which is the lazy per-selection fetch a client makes.
    let newest = call(
        &node,
        &p,
        ws,
        "versions.get",
        json!({ "kind": "dashboard", "id": "ops", "version_id": rows[0]["version_id"] }),
    )
    .await;
    assert_eq!(newest["snapshot"]["title"], json!("Ops 25"));
    let oldest = call(
        &node,
        &p,
        ws,
        "versions.get",
        json!({ "kind": "dashboard", "id": "ops", "version_id": rows[19]["version_id"] }),
    )
    .await;
    assert_eq!(
        oldest["snapshot"]["title"],
        json!("Ops 6"),
        "saves 1..=5 were trimmed, oldest-first"
    );

    // Provenance is on every row: who, with which verb, and when (epoch MILLIS from the row's ULID).
    assert_eq!(rows[0]["actor"], json!("user:test"));
    assert_eq!(rows[0]["tool"], json!("dashboard.save"));
    assert!(
        rows[0]["ts"].as_u64().expect("a ts") > 1_600_000_000_000,
        "ts is unix millis, decoded from the row's own ULID"
    );
    assert_eq!(
        rows[0]["is_head"],
        json!(true),
        "the newest version matches the live record, so it is marked current"
    );
    assert_eq!(rows[1]["is_head"], json!(false));
}

/// The capped-insert key lock, exercised through the real dispatch path: N concurrent saves of ONE
/// entity must leave the ring at exactly the cap, never `cap + k`. This is the property
/// `capped.rs`'s per-key lock exists for; here it is proven at the seam that actually uses it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_saves_of_one_entity_never_over_grow_the_ring() {
    let ws = "ver-race";
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = member(ws);

    // Seed past the cap serially first, so every concurrent save below is a TRIM, which is the
    // racing operation.
    for i in 1..=20u64 {
        call(
            &node,
            &p,
            ws,
            "dashboard.save",
            save_dashboard("race", &format!("S{i}"), i),
        )
        .await;
    }

    let mut tasks = Vec::new();
    for i in 21..=40u64 {
        let node = node.clone();
        let p = p.clone();
        let ws = ws.to_string();
        tasks.push(tokio::spawn(async move {
            // A distinct title per save so nothing is deduped away — every one of these must
            // genuinely contend for the ring.
            let _ = call_tool(
                &node,
                &p,
                &ws,
                "dashboard.save",
                &save_dashboard("race", &format!("S{i}"), i).to_string(),
            )
            .await;
        }));
    }
    for t in tasks {
        t.await.expect("save task completes");
    }

    let rows = list(&node, &p, ws, "dashboard", "race").await;
    assert_eq!(
        rows.len(),
        20,
        "concurrent saves must leave the ring at exactly the cap — over-growth means the \
         insert+trim lost its per-key serialization"
    );
}

/// A no-op save (identical content) must not burn a ring slot — otherwise a client that re-saves on
/// every blur silently pushes the real history off the end.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_identical_save_is_deduped_by_snapshot_hash() {
    let ws = "ver-dedupe";
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = member(ws);

    call(
        &node,
        &p,
        ws,
        "dashboard.save",
        save_dashboard("d", "One", 1),
    )
    .await;
    let after_first = list(&node, &p, ws, "dashboard", "d").await.len();

    // The SAME content again. `now` differs, but `now` is not stored as content the save changes —
    // if it were, this assertion would catch that too, which is the point.
    call(
        &node,
        &p,
        ws,
        "dashboard.save",
        save_dashboard("d", "One", 2),
    )
    .await;
    call(
        &node,
        &p,
        ws,
        "dashboard.save",
        save_dashboard("d", "One", 3),
    )
    .await;
    assert_eq!(
        list(&node, &p, ws, "dashboard", "d").await.len(),
        after_first,
        "identical snapshots are deduped against the ring head"
    );

    // A REAL change is not deduped.
    call(
        &node,
        &p,
        ws,
        "dashboard.save",
        save_dashboard("d", "Two", 4),
    )
    .await;
    assert_eq!(
        list(&node, &p, ws, "dashboard", "d").await.len(),
        after_first + 1
    );
}

/// The cap is adjustable per workspace AND per kind, and a lowered cap applies on the next capture
/// (capped_insert trims to whatever cap it is handed — no reaper to run).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_per_kind_cap_override_is_honoured_by_capture() {
    let ws = "ver-cap";
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = member(ws);

    let cfg = call(
        &node,
        &p,
        ws,
        "versions.config.set",
        json!({ "cap": 10, "per_kind": { "dashboard": 3 } }),
    )
    .await;
    assert_eq!(cfg["cap"], json!(10));
    assert_eq!(cfg["per_kind"]["dashboard"], json!(3));
    assert_eq!(
        cfg["max_cap"],
        json!(100),
        "the node's bounds travel with the config"
    );

    for i in 1..=8u64 {
        call(
            &node,
            &p,
            ws,
            "dashboard.save",
            save_dashboard("c", &format!("T{i}"), i),
        )
        .await;
    }
    let rows = list(&node, &p, ws, "dashboard", "c").await;
    assert_eq!(
        rows.len(),
        3,
        "the per-kind override wins over the workspace cap"
    );

    // A cap outside the node's clamp is a REJECTION, not a silent clamp.
    let err = try_call(&node, &p, ws, "versions.config.set", json!({ "cap": 500 }))
        .await
        .expect_err("500 is above the node ceiling");
    assert!(err.contains("100"), "the refusal names the ceiling: {err}");

    // An unknown kind in per_kind is rejected rather than stored as a typo that does nothing.
    let err = try_call(
        &node,
        &p,
        ws,
        "versions.config.set",
        json!({ "per_kind": { "dashbored": 5 } }),
    )
    .await
    .expect_err("an unknown kind is refused");
    assert!(
        err.contains("dashbored"),
        "the refusal names the typo: {err}"
    );
}

// ---------------------------------------------------------------------------------------------
// Restore
// ---------------------------------------------------------------------------------------------

/// Restore round-trip, kind 1 of 3: a dashboard's content comes back byte-for-byte, and the restore
/// appends a NEW HEAD equal to the restored version (history stays append-only and re-restorable).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn restoring_a_dashboard_round_trips_and_appends_a_new_head() {
    let ws = "ver-restore-dash";
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = member(ws);

    call(
        &node,
        &p,
        ws,
        "dashboard.save",
        json!({ "id": "pr", "title": "Good", "cells": [{ "i": "c1", "x": 0, "y": 0, "w": 6, "h": 4, "view": "timeseries" }], "now": 1 }),
    )
    .await;
    call(
        &node,
        &p,
        ws,
        "dashboard.save",
        json!({ "id": "pr", "title": "Wrecked", "cells": [], "now": 2 }),
    )
    .await;

    let rows = list(&node, &p, ws, "dashboard", "pr").await;
    assert_eq!(rows.len(), 2);
    let good = rows[1]["version_id"]
        .as_str()
        .expect("the older version")
        .to_string();

    let out = call(
        &node,
        &p,
        ws,
        "versions.restore",
        json!({ "kind": "dashboard", "id": "pr", "version_id": good, "now": 3 }),
    )
    .await;
    assert_eq!(out["ok"], json!(true));
    assert_eq!(out["restored_from"], json!(good));

    // The LIVE record is the old content again.
    let live = call(&node, &p, ws, "dashboard.get", json!({ "id": "pr" })).await;
    assert_eq!(live["title"], json!("Good"));
    assert_eq!(live["cells"].as_array().expect("cells").len(), 1);

    // ...and the restore appended a new head equal to it (the scope's append-only property).
    let rows = list(&node, &p, ws, "dashboard", "pr").await;
    assert_eq!(
        rows.len(),
        3,
        "a restore appends a version, it never rewrites one"
    );
    assert_eq!(
        rows[0]["tool"],
        json!("versions.restore"),
        "the new head records that it came from a restore, not a hand save"
    );
    assert_eq!(
        rows[0]["hash"], rows[2]["hash"],
        "the new head has the same content as the version that was restored"
    );
    assert_eq!(rows[0]["is_head"], json!(true));
}

/// Restore round-trip, kind 2 of 3: a flow. Its `version` counter keeps CLIMBING (run-pinning
/// semantics, flows-scope Decision 1) — restoring v1 does not rewind the counter — while the ring
/// carries which counter value each snapshot had, which is what makes "v12" inspectable.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn restoring_a_flow_round_trips_and_the_counter_keeps_climbing() {
    let ws = "ver-restore-flow";
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = member(ws);

    let flow = |name: &str| {
        json!({
            "id": "f1",
            "name": name,
            "workspace": ws,
            "nodes": [{ "id": "n1", "type": "debug", "config": {}, "position": { "x": 0, "y": 0 } }],
            "edges": []
        })
    };
    call(&node, &p, ws, "flows.save", flow("First")).await;
    call(&node, &p, ws, "flows.save", flow("Second")).await;
    call(&node, &p, ws, "flows.save", flow("Third")).await;

    let rows = list(&node, &p, ws, "flow", "f1").await;
    assert_eq!(rows.len(), 3);
    assert_eq!(
        rows[0]["entity_version"],
        json!(3),
        "each ring row carries the flow's own run-pinning counter — this is the UI's `v` column"
    );
    assert_eq!(rows[2]["entity_version"], json!(1));

    let first = rows[2]["version_id"].as_str().expect("v1").to_string();
    call(
        &node,
        &p,
        ws,
        "versions.restore",
        json!({ "kind": "flow", "id": "f1", "version_id": first }),
    )
    .await;

    let live = call(&node, &p, ws, "flows.get", json!({ "id": "f1" })).await;
    assert_eq!(live["name"], json!("First"), "the old graph is live again");
    assert_eq!(
        live["version"],
        json!(4),
        "the counter CLIMBS on a restore — a live run keeps whatever version it pinned"
    );
}

/// Restore round-trip, kind 3 of 3: a rule — and the scope's restore-AFTER-DELETE case. The ring
/// outlives the entity (there is no capture on delete), so restoring the head re-creates it.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn restoring_a_deleted_rule_recreates_it() {
    let ws = "ver-restore-rule";
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = member(ws);

    call(
        &node,
        &p,
        ws,
        "rules.save",
        json!({ "id": "high-temp", "name": "High temp", "body": "let x = 1; x" }),
    )
    .await;

    let rows = list(&node, &p, ws, "rule", "high-temp").await;
    assert_eq!(
        rows.len(),
        1,
        "a delete captures nothing — the ring already holds the last save"
    );
    let head = rows[0]["version_id"].as_str().expect("head").to_string();

    call(&node, &p, ws, "rules.delete", json!({ "id": "high-temp" })).await;
    assert!(
        try_call(&node, &p, ws, "rules.get", json!({ "id": "high-temp" }))
            .await
            .is_err(),
        "the rule is gone"
    );

    // The history is STILL readable for a deleted entity — that is what makes this recoverable.
    let rows = list(&node, &p, ws, "rule", "high-temp").await;
    assert_eq!(rows.len(), 1, "the ring outlives the entity");
    assert_eq!(
        rows[0]["is_head"],
        json!(false),
        "with no live record, nothing is marked current"
    );

    call(
        &node,
        &p,
        ws,
        "versions.restore",
        json!({ "kind": "rule", "id": "high-temp", "version_id": head }),
    )
    .await;

    let live = call(&node, &p, ws, "rules.get", json!({ "id": "high-temp" })).await;
    assert_eq!(live["name"], json!("High temp"));
    assert_eq!(live["body"], json!("let x = 1; x"));
}

/// Restore is a FORWARD action through the kind's own save verb, so it inherits that verb's
/// validators: a snapshot the current rules refuse is REFUSED, not written. Proven with a flow whose
/// stored graph is made invalid by hand (a dangling edge) — the same shape a since-tightened
/// validator produces.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_snapshot_the_validators_now_refuse_is_not_written() {
    let ws = "ver-validate";
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = member(ws);

    let good = json!({
        "id": "f2", "name": "Good", "workspace": ws,
        "nodes": [{ "id": "n1", "type": "debug", "config": {}, "position": { "x": 0, "y": 0 } }],
        "edges": []
    });
    call(&node, &p, ws, "flows.save", good).await;
    let rows = list(&node, &p, ws, "flow", "f2").await;
    let version_id = rows[0]["version_id"].as_str().expect("v1").to_string();

    // Corrupt the STORED SNAPSHOT to a graph the DAG validator rejects (an edge to a node that does
    // not exist). Writing the ring row directly is legitimate here and only here: the point is to
    // simulate a snapshot that WAS valid when captured and is not valid now, which no sequence of
    // real calls can produce inside one test.
    let key = "flow:f2";
    let mut row: serde_json::Map<String, Value> = serde_json::from_value(
        call(
            &node,
            &p,
            ws,
            "versions.get",
            json!({ "kind": "flow", "id": "f2", "version_id": version_id }),
        )
        .await["snapshot"]
            .clone(),
    )
    .expect("the snapshot is an object");
    // A dependency on a node that does not exist — `DagError::UnknownDependency`, which
    // `flows.save` raises BEFORE any write. (Flow topology is `nodes[].needs`, not an `edges` list.)
    row.insert(
        "nodes".into(),
        json!([{ "id": "n1", "type": "debug", "config": {}, "needs": ["ghost"], "position": { "x": 0, "y": 0 } }]),
    );
    poison_snapshot(&node, ws, &version_id, key, Value::Object(row)).await;

    let err = try_call(
        &node,
        &p,
        ws,
        "versions.restore",
        json!({ "kind": "flow", "id": "f2", "version_id": version_id }),
    )
    .await
    .expect_err("an invalid snapshot must be refused, not written");
    assert!(
        err.contains("flows.save"),
        "the refusal names the verb whose validator refused: {err}"
    );

    // The LIVE flow is untouched — a refused restore writes nothing.
    let live = call(&node, &p, ws, "flows.get", json!({ "id": "f2" })).await;
    assert_eq!(live["name"], json!("Good"));
    assert!(
        live["nodes"][0]["needs"]
            .as_array()
            .expect("the live node's needs")
            .is_empty(),
        "the live graph never grew the dangling dependency the refused snapshot carried"
    );
    assert_eq!(
        live["version"],
        json!(1),
        "a refused restore did not bump the counter either"
    );
}

/// Overwrite one ring row's `snapshot` in place, preserving the fields `capped_insert` injected.
/// Test-only surgery — see the caller for why it is the only way to express "a snapshot that has
/// since become invalid".
async fn poison_snapshot(
    node: &Arc<Node>,
    ws: &str,
    version_id: &str,
    cap_key: &str,
    snapshot: Value,
) {
    node.store
        .query_ws(
            ws,
            "UPDATE type::record($tb, $id) MERGE { snapshot: $snapshot, cap_key: $key, seq: $id }",
            vec![
                (
                    "tb".into(),
                    Value::String(lb_host::ENTITY_VERSION_TABLE.to_string()),
                ),
                ("id".into(), Value::String(version_id.to_string())),
                ("key".into(), Value::String(cap_key.to_string())),
                ("snapshot".into(), snapshot),
            ],
        )
        .await
        .expect("the ring row is updated");
}

// ---------------------------------------------------------------------------------------------
// Catalog
// ---------------------------------------------------------------------------------------------

/// The verbs are advertised AND their args validate — the scope's catalog requirement. A verb in the
/// catalog with no schema is how a model ends up guessing argument names turn after turn.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn versions_verbs_are_in_the_catalog_with_validating_args() {
    let ws = "ver-catalog";
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = member(ws);

    let cat = call(&node, &p, ws, "tools.catalog", json!({})).await;
    let text = cat.to_string();
    for verb in [
        "versions.list",
        "versions.get",
        "versions.restore",
        "versions.config.get",
        "versions.config.set",
    ] {
        assert!(
            text.contains(verb),
            "{verb} is missing from the tool catalog"
        );
    }

    // A bad `kind` is a typed ARGUMENT error at the validator — not an empty list a caller would
    // read as "this entity has no history".
    let err = try_call(
        &node,
        &p,
        ws,
        "versions.list",
        json!({ "kind": 7, "id": "x" }),
    )
    .await
    .expect_err("a non-string kind fails validation");
    assert!(err.contains("kind"), "the error names the bad arg: {err}");

    // An unknown (but well-typed) kind names the kinds that DO exist.
    let err = try_call(
        &node,
        &p,
        ws,
        "versions.list",
        json!({ "kind": "widget", "id": "x" }),
    )
    .await
    .expect_err("an unknown kind is refused");
    assert!(
        err.contains("dashboard") && err.contains("flow") && err.contains("rule"),
        "the refusal lists the versioned kinds: {err}"
    );

    // A version id from ANOTHER entity is not usable on this one — the entity scoping in
    // `read_version` is what stops a restore grant on one record driving a save on another.
    call(&node, &p, ws, "dashboard.save", save_dashboard("a", "A", 1)).await;
    call(&node, &p, ws, "dashboard.save", save_dashboard("b", "B", 1)).await;
    let a_version = list(&node, &p, ws, "dashboard", "a").await[0]["version_id"]
        .as_str()
        .expect("a version")
        .to_string();
    let err = try_call(
        &node,
        &p,
        ws,
        "versions.get",
        json!({ "kind": "dashboard", "id": "b", "version_id": a_version }),
    )
    .await
    .expect_err("a's version is not b's");
    assert!(
        err.contains("no such tool"),
        "a cross-entity version id is an opaque NotFound — the same shape an unknown verb has, so \
         it leaks nothing about what exists elsewhere: {err}"
    );
}
