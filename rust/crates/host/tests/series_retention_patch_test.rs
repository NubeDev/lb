//! `series.retention.patch` + policy PROVENANCE (series-observability follow-up).
//!
//! **The bug both close.** `series.retention.set` replaces the whole row, so a hand-written body
//! silently drops any field its author forgot — a live `modbus.` policy lost its tier `method`
//! exactly that way — and nothing recorded that it had happened, so "who did this?" was answerable
//! only by eliminating every writer across three repos.
//!
//! `patch` is the merge-preserving write; `updated_by`/`updated_ms` are stamped host-side so the
//! question has an answer next time. Real `Store::memory()`, real MCP dispatch through
//! `call_ingest_tool`. No mocks.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_ingest_tool, call_tool, Node};
use lb_mcp::ToolError;
use lb_store::Store;
use serde_json::{json, Value};
use std::sync::Arc;

const SET: &str = "mcp:series.retention.set:call";
const LIST: &str = "mcp:series.retention.list:call";
const PREFIX: &str = "modbus.";

fn principal(sub: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: "nube".into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

async fn call(store: &Store, p: &Principal, tool: &str, args: Value) -> Result<Value, ToolError> {
    call_ingest_tool(store, p, "nube", tool, &args).await
}

/// The policy the reported bug started from: a real tier, with a real method.
async fn seed(store: &Store, p: &Principal) {
    call(
        store,
        p,
        "series.retention.set",
        json!({
            "prefix": PREFIX,
            "raw_for_ms": 3_600_000,
            "max_samples": 5_000,
            "tiers": [{ "width_ms": 60_000, "keep_for_ms": 604_800_000, "method": "avg" }],
            "now_ms": 1_700_000_000_000u64,
        }),
    )
    .await
    .expect("seed set");
}

async fn policy(store: &Store, p: &Principal) -> Value {
    let out = call(store, p, "series.retention.list", json!({}))
        .await
        .expect("list");
    out["policies"]
        .as_array()
        .expect("policies is an array")
        .iter()
        .find(|r| r["prefix"] == PREFIX)
        .cloned()
        .expect("the seeded policy exists")
}

// ------------------------------------------------------------------------------- provenance ----

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_write_records_who_did_it_and_when() {
    let store = Store::memory().await.unwrap();
    let test = principal("user:test", &[SET, LIST]);
    seed(&store, &test).await;

    let p = policy(&store, &test).await;
    assert_eq!(p["updated_by"], "user:test");
    assert_eq!(p["updated_ms"], 1_700_000_000_000u64);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn provenance_cannot_be_forged_by_the_caller() {
    // THE load-bearing assertion. `Policy` has no `deny_unknown_fields`, and a real read-modify-write
    // client (modbus's settings panel) spreads the host's own reply straight back into `set` — so an
    // echoed `updated_by` DOES arrive on the wire. If the stamp were conditional, the row would
    // record whoever the caller claimed, and the field would answer nothing. It is overwritten
    // unconditionally, exactly as the producer root on `ingest.write` is.
    let store = Store::memory().await.unwrap();
    let test = principal("user:test", &[SET, LIST]);

    call(
        &store,
        &test,
        "series.retention.set",
        json!({
            "prefix": PREFIX,
            "raw_for_ms": 60_000,
            "max_samples": 0,
            "tiers": [],
            "updated_by": "user:someone-else",
            "updated_ms": 1u64,
            "now_ms": 1_700_000_000_000u64,
        }),
    )
    .await
    .expect("set accepts and ignores the forged provenance");

    let p = policy(&store, &test).await;
    assert_eq!(
        p["updated_by"], "user:test",
        "the forged author was written"
    );
    assert_ne!(p["updated_ms"], 1, "the forged timestamp was written");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_row_written_before_provenance_existed_reports_nothing_rather_than_guessing() {
    // An upgraded node's rows have no author. `None` is the truth; a fabricated one would be worse,
    // and the explicit projection in `list_policies` returns the absent column as a present NULL —
    // which `Option` handles and a bare value would have errored on (aborting the whole GC pass).
    let store = Store::memory().await.unwrap();
    lb_ingest::set_policy(
        &store,
        "nube",
        &lb_ingest::Policy {
            prefix: PREFIX.into(),
            raw_for_ms: 60_000,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let p = policy(&store, &principal("user:test", &[SET, LIST])).await;
    assert_eq!(p.get("updated_by"), None, "absent, not invented");
}

// ------------------------------------------------------------------------------ merge rules ----

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn patching_one_field_leaves_every_other_alone() {
    let store = Store::memory().await.unwrap();
    let test = principal("user:test", &[SET, LIST]);
    seed(&store, &test).await;

    call(
        &store,
        &test,
        "series.retention.patch",
        json!({ "prefix": PREFIX, "raw_for_ms": 7_200_000 }),
    )
    .await
    .expect("patch");

    let p = policy(&store, &test).await;
    assert_eq!(p["raw_for_ms"], 7_200_000);
    assert_eq!(p["max_samples"], 5_000, "an untouched field changed");
    assert_eq!(
        p["tiers"][0]["method"], "avg",
        "the tier method was dropped"
    );
    assert_eq!(p["tiers"][0]["keep_for_ms"], 604_800_000u64);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn re_sending_a_tier_without_its_method_keeps_the_method() {
    // THE reported bug, as a test. A caller re-sends the tier to change its width and omits
    // `method`; under `set` that silently erases it. Under `patch` the tier is merged field-wise
    // with the stored tier of the same width.
    let store = Store::memory().await.unwrap();
    let test = principal("user:test", &[SET, LIST]);
    seed(&store, &test).await;

    call(
        &store,
        &test,
        "series.retention.patch",
        json!({ "prefix": PREFIX, "tiers": [{ "width_ms": 60_000, "keep_for_ms": 999 }] }),
    )
    .await
    .expect("patch");

    let p = policy(&store, &test).await;
    assert_eq!(p["tiers"][0]["keep_for_ms"], 999);
    assert_eq!(
        p["tiers"][0]["method"], "avg",
        "the method was silently dropped"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_same_omission_through_set_still_drops_it() {
    // The contrast that makes `patch` worth having — and proof `set` keeps its replace semantics
    // (removing a tier or a filter has to remain possible).
    let store = Store::memory().await.unwrap();
    let test = principal("user:test", &[SET, LIST]);
    seed(&store, &test).await;

    call(
        &store,
        &test,
        "series.retention.set",
        json!({
            "prefix": PREFIX,
            "raw_for_ms": 3_600_000,
            "max_samples": 5_000,
            "tiers": [{ "width_ms": 60_000, "keep_for_ms": 999 }],
        }),
    )
    .await
    .expect("set");

    let p = policy(&store, &test).await;
    assert_eq!(p["tiers"][0].get("method"), None, "set must still REPLACE");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_tier_can_still_be_removed_and_a_method_explicitly_cleared() {
    let store = Store::memory().await.unwrap();
    let test = principal("user:test", &[SET, LIST]);
    seed(&store, &test).await;

    // An explicit null clears; only ABSENCE preserves. Without the distinction one of the two would
    // be impossible, which is why the merge reads raw JSON rather than a typed struct.
    call(
        &store,
        &test,
        "series.retention.patch",
        json!({ "prefix": PREFIX, "tiers": [{ "width_ms": 60_000, "method": null }] }),
    )
    .await
    .expect("patch");
    assert_eq!(policy(&store, &test).await["tiers"][0].get("method"), None);

    call(
        &store,
        &test,
        "series.retention.patch",
        json!({ "prefix": PREFIX, "tiers": [] }),
    )
    .await
    .expect("patch");
    assert_eq!(
        policy(&store, &test).await["tiers"]
            .as_array()
            .unwrap()
            .len(),
        0,
        "an empty tier list must still remove every tier"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_write_returns_what_was_actually_stored() {
    // So a caller SEES what a replace cost them, instead of discovering it in a panel days later.
    let store = Store::memory().await.unwrap();
    let test = principal("user:test", &[SET, LIST]);
    seed(&store, &test).await;

    let out = call(
        &store,
        &test,
        "series.retention.patch",
        json!({ "prefix": PREFIX, "max_samples": 10_000 }),
    )
    .await
    .expect("patch");

    assert_eq!(out["policy"]["max_samples"], 10_000);
    assert_eq!(out["policy"]["tiers"][0]["method"], "avg");
    assert_eq!(out["policy"]["updated_by"], "user:test");
}

// ----------------------------------------------------------------------------------- guards ----

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn patching_a_prefix_with_no_policy_is_an_error_not_a_silent_create() {
    let store = Store::memory().await.unwrap();
    let test = principal("user:test", &[SET, LIST]);
    let err = call(
        &store,
        &test,
        "series.retention.patch",
        json!({ "prefix": "nothing.here.", "raw_for_ms": 1 }),
    )
    .await
    .unwrap_err();
    // Inventing a row from partial fields is how a half-configured policy gets written.
    assert!(matches!(err, ToolError::BadInput(_)), "got {err:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_empty_prefix_is_refused() {
    // `resolve_policy` matches on `starts_with`, so an empty prefix silently governs EVERY series.
    // It became reachable when `Policy` gained `Default`.
    let store = Store::memory().await.unwrap();
    let test = principal("user:test", &[SET, LIST]);
    let err = call(
        &store,
        &test,
        "series.retention.set",
        json!({ "prefix": "", "raw_for_ms": 1, "max_samples": 0, "tiers": [] }),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::BadInput(_)), "got {err:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn patch_is_refused_without_the_set_capability() {
    // It rides `mcp:series.retention.set:call` — the same administrative privilege, no new cap.
    let store = Store::memory().await.unwrap();
    let test = principal("user:test", &[SET, LIST]);
    seed(&store, &test).await;

    let reader = principal("user:bob", &[LIST]);
    let err = call(
        &store,
        &reader,
        "series.retention.patch",
        json!({ "prefix": PREFIX, "max_samples": 1 }),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::Denied), "got {err:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn changing_a_tier_width_inherits_the_method_rather_than_losing_it() {
    // Found on a LIVE node, not in review: width is the merge identity, so retuning a tier from 5
    // minutes to 1 minute has no stored counterpart to merge with — and the method vanished. That is
    // the reported bug in a different hat.
    //
    // The inheritance rule is lb's own: a method is a property of the SERIES' meaning, not of a
    // width (`Policy::method_for`), which is already how a bucketed READ resolves one. A coil
    // configured `last` must not silently become an average because someone retuned the bucket size.
    let store = Store::memory().await.unwrap();
    let test = principal("user:test", &[SET, LIST]);
    seed(&store, &test).await; // one 60s tier, method avg

    call(
        &store,
        &test,
        "series.retention.patch",
        json!({ "prefix": PREFIX, "tiers": [{ "width_ms": 300_000, "keep_for_ms": 604_800_000 }] }),
    )
    .await
    .expect("patch");

    let p = policy(&store, &test).await;
    assert_eq!(p["tiers"][0]["width_ms"], 300_000);
    assert_eq!(
        p["tiers"][0]["method"], "avg",
        "retuning the bucket width silently dropped the method"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_new_tier_on_a_policy_with_no_method_anywhere_stays_method_less() {
    // Inheritance must not INVENT a method: if nothing in the policy declares one, the new tier has
    // none either. Fabricating `avg` would be the fabricated-healthy class of bug all over again.
    let store = Store::memory().await.unwrap();
    let test = principal("user:test", &[SET, LIST]);
    call(
        &store,
        &test,
        "series.retention.set",
        json!({ "prefix": PREFIX, "raw_for_ms": 60_000, "max_samples": 0,
                "tiers": [{ "width_ms": 60_000, "keep_for_ms": 1 }] }),
    )
    .await
    .expect("set");

    call(
        &store,
        &test,
        "series.retention.patch",
        json!({ "prefix": PREFIX, "tiers": [{ "width_ms": 900_000, "keep_for_ms": 1 }] }),
    )
    .await
    .expect("patch");

    assert_eq!(policy(&store, &test).await["tiers"][0].get("method"), None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn delete_is_reachable_through_the_real_dispatcher() {
    // REGRESSION, found by driving it on a live node. `series_retention_delete` deliberately mints
    // no cap of its own — its service layer re-checks `series.retention.set` — but the OUTER gate in
    // `tool_call` was demanding `mcp:series.retention.delete:call`, which no role bundle grants. The
    // verb was therefore unreachable for EVERY caller since it shipped.
    //
    // Every existing test called the host fn or `call_ingest_tool` directly and so never crossed the
    // outer gate, which is exactly why nothing caught it. This one goes through `call_tool`.
    let node = Arc::new(Node::boot().await.unwrap());
    let test = principal("user:test", &[SET, LIST]);

    call_tool(
        &node,
        &test,
        "nube",
        "series.retention.set",
        &json!({ "prefix": PREFIX, "raw_for_ms": 60_000, "max_samples": 0, "tiers": [] })
            .to_string(),
    )
    .await
    .expect("set through the real dispatcher");

    call_tool(
        &node,
        &test,
        "nube",
        "series.retention.delete",
        &json!({ "prefix": PREFIX }).to_string(),
    )
    .await
    .expect("delete must be reachable with the SET capability");

    let out = call_tool(&node, &test, "nube", "series.retention.list", "{}")
        .await
        .expect("list");
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        v["policies"].as_array().unwrap().len(),
        0,
        "the policy survived delete"
    );
}
