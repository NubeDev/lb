//! `analysis` — the finding's statement of the producer's REASONING, over a REAL booted `Node`
//! (`docs/scope/insights/insight-analysis-scope.md`). Real store, real bus, real caps, the real
//! `call_tool` MCP bridge. NO mocks (CLAUDE §9): records are seeded by raising through the verb
//! under test and read back through it.
//!
//! Mandatory categories: capability-deny (analysis must not open an alternate read path) +
//! workspace-isolation. Scope-named cases: round-trip, the three `Quantity` shapes (incl. the
//! number-not-string assertion the sortable corpus depends on), the migration guard,
//! dedup-refresh + the omit arm, the 4 KB reject with no orphan row, the get-vs-list boundary, and
//! the deliberate closed-struct DROP of an unknown key.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, Node};
use lb_mcp::ToolError;
use serde_json::{json, Value};

const RAISE: &str = "mcp:insight.raise:call";
const GET: &str = "mcp:insight.get:call";
const LIST: &str = "mcp:insight.list:call";

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

/// The full six-field fixture, modelled on the worked example that motivated the scope: a finding
/// whose producer computed no numbers but recorded *why* there aren't any (the note-only
/// `Quantity`), which a bare `Option<f64>` would have forced it to drop.
fn analysis() -> Value {
    json!({
        "trigger_logic": "Zero consumption for 24 consecutive hours",
        "suspected_cause": "Meter offline or site unoccupied (weekend)",
        "normalised_metric": "Daily usage (kL)",
        "benchmark_context": "vs expected minimum baseline",
        "deviation": { "note": "N/A" },
        "estimated_impact": { "note": "N/A (data quality)" },
    })
}

fn raise_input(dedup_key: &str, ts: u64, an: Option<Value>) -> Value {
    let mut v = json!({
        "dedup_key": dedup_key,
        "severity": "warning",
        "title": "no usage in 1 day",
        "body": { "reading": 0.0 },
        "origin": { "kind": "rule", "ref": "rule:no-usage-1d" },
        "ts": ts,
    });
    if let Some(an) = an {
        v["analysis"] = an;
    }
    v
}

// --- round-trip: what the producer stated is what `insight.get` echoes ---------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn get_echoes_the_analysis_the_producer_stated() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:test", "nube", &[RAISE, GET]);
    let out = call(
        &node,
        &p,
        "nube",
        "insight.raise",
        raise_input("k1", 1, Some(analysis())),
    )
    .await
    .expect("raise ok");
    let id = out["id"].as_str().unwrap();

    let got = call(&node, &p, "nube", "insight.get", json!({ "id": id }))
        .await
        .expect("get ok");
    assert_eq!(
        got["analysis"],
        analysis(),
        "analysis round-trips byte-identically"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_producer_that_knows_one_field_states_one_field() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:test", "nube", &[RAISE, GET]);
    let out = call(
        &node,
        &p,
        "nube",
        "insight.raise",
        raise_input("k", 1, Some(json!({ "trigger_logic": "flat for 24h" }))),
    )
    .await
    .expect("raise ok");
    let id = out["id"].as_str().unwrap();

    let got = call(&node, &p, "nube", "insight.get", json!({ "id": id }))
        .await
        .expect("get ok");
    assert_eq!(got["analysis"]["trigger_logic"], "flat for 24h");
    // The other five are ABSENT, not empty strings — `skip_serializing_if` — so a consumer can
    // distinguish "not stated" from "stated as nothing" and render only the labels that have text.
    for absent in [
        "suspected_cause",
        "normalised_metric",
        "benchmark_context",
        "deviation",
        "estimated_impact",
    ] {
        assert!(
            got["analysis"].get(absent).is_none(),
            "unstated `{absent}` serializes to nothing: {}",
            got["analysis"]
        );
    }
}

// --- `Quantity` in all three shapes; the number stays a NUMBER ---------------------------------
// The whole reason `deviation`/`estimated_impact` are a typed quantity rather than prose is that
// "rank today's findings by cost" must work off the stored corpus. That dies silently if a value
// lands as a quoted string, so the decode-as-number assertion is the one that proves the corpus is
// queryable — not merely that the field round-trips.

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn quantity_round_trips_note_only_value_unit_and_all_three() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:test", "nube", &[RAISE, GET]);
    let out = call(
        &node,
        &p,
        "nube",
        "insight.raise",
        raise_input(
            "k",
            1,
            Some(json!({
                // note-only — the honest "we considered it and it doesn't apply"
                "deviation": { "note": "N/A" },
                // value + unit + note — the sortable number WITH context
                "estimated_impact": { "value": 180.0, "unit": "AUD/day", "note": "vs 1.8 kL baseline" },
            })),
        ),
    )
    .await
    .expect("raise ok");
    let id = out["id"].as_str().unwrap();

    let got = call(&node, &p, "nube", "insight.get", json!({ "id": id }))
        .await
        .expect("get ok");
    let dev = &got["analysis"]["deviation"];
    assert_eq!(dev["note"], "N/A");
    assert!(
        dev.get("value").is_none() && dev.get("unit").is_none(),
        "a note-only quantity stores no number: {dev}"
    );

    let impact = &got["analysis"]["estimated_impact"];
    assert_eq!(
        impact["value"].as_f64(),
        Some(180.0),
        "the value decodes as a NUMBER — this is the sortability the type exists for: {impact}"
    );
    assert!(
        impact["value"].is_number() && !impact["value"].is_string(),
        "never a stringified number: {impact}"
    );
    assert_eq!(impact["unit"], "AUD/day");
    assert_eq!(impact["note"], "vs 1.8 kL baseline");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_value_without_a_unit_is_rejected() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:test", "nube", &[RAISE, LIST]);
    // A bare number whose unit nobody recorded is the seed of the cross-producer unit-mismatch bug:
    // it cannot be compared or summed, and by the time a consumer notices, the corpus is written.
    let r = call(
        &node,
        &p,
        "nube",
        "insight.raise",
        raise_input("k", 1, Some(json!({ "deviation": { "value": -100.0 } }))),
    )
    .await;
    assert!(
        matches!(r, Err(ToolError::BadInput(_))),
        "a value with no unit rejects: {r:?}"
    );
    // Rejected UP FRONT, like every other guard on this verb — no orphan parent row.
    let page = call(&node, &p, "nube", "insight.list", json!({}))
        .await
        .expect("list ok");
    assert_eq!(page["items"].as_array().unwrap().len(), 0, "no orphan row");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_all_absent_quantity_is_rejected_rather_than_stored_empty() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:test", "nube", &[RAISE]);
    // `{}` says strictly less than omitting the field, and a consumer rendering "Deviation: —"
    // for it would be inventing a distinction the producer never made.
    let r = call(
        &node,
        &p,
        "nube",
        "insight.raise",
        raise_input("k", 1, Some(json!({ "estimated_impact": {} }))),
    )
    .await;
    assert!(
        matches!(r, Err(ToolError::BadInput(_))),
        "an empty quantity rejects: {r:?}"
    );
}

// --- THE MIGRATION GUARD ---------------------------------------------------------------------
// `insight.list` decodes with `filter_map(|v| from_value(v).ok())` — a record that fails to decode
// is DROPPED with no error anywhere. `analysis` is `Option` + `#[serde(default)]` +
// `skip_serializing_if`, so a raise that states none stores a blob with NO `analysis` key —
// byte-identical to every record written before the field existed. If the field were ever made
// required, this catches it as "the roster is empty" rather than as a decode error.

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_record_with_no_analysis_key_still_lists_and_gets() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:test", "nube", &[RAISE, GET, LIST]);
    let out = call(
        &node,
        &p,
        "nube",
        "insight.raise",
        raise_input("legacy", 1, None),
    )
    .await
    .expect("raise ok");
    let id = out["id"].as_str().unwrap();

    let page = call(&node, &p, "nube", "insight.list", json!({}))
        .await
        .expect("list ok");
    let items = page["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "a pre-field-shaped record still lists");
    assert_eq!(items[0]["dedup_key"], "legacy");

    let got = call(&node, &p, "nube", "insight.get", json!({ "id": id }))
        .await
        .expect("get ok");
    assert!(
        got.get("analysis").is_none(),
        "absent analysis serializes to nothing, not null: {got}"
    );
}

// --- dedup: analysis REFRESHES (and an omitting raise does not blank it) ------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn re_raise_refreshes_analysis_and_omitting_it_leaves_the_stored_value() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:test", "nube", &[RAISE, GET]);

    let first = call(
        &node,
        &p,
        "nube",
        "insight.raise",
        raise_input("k", 1, Some(analysis())),
    )
    .await
    .expect("raise 1");
    let id = first["id"].as_str().unwrap().to_string();

    // The rule is improved to compute a real baseline and re-raises the same key. The stored
    // reasoning must REFRESH: a `"N/A"` deviation from firing #1 shown beside `count: 2` is worse
    // than absent — it is the drawer confidently describing a firing that is no longer the finding.
    let second = raise_input(
        "k",
        2,
        Some(json!({
            "trigger_logic": "Zero consumption for 24 consecutive hours",
            "deviation": { "value": -100.0, "unit": "%", "note": "vs 1.8 kL baseline" },
        })),
    );
    call(&node, &p, "nube", "insight.raise", second)
        .await
        .expect("raise 2");

    let got = call(&node, &p, "nube", "insight.get", json!({ "id": &id }))
        .await
        .expect("get ok");
    assert_eq!(got["count"], 2, "dedup bumped, not duplicated");
    assert_eq!(
        got["analysis"]["deviation"]["value"].as_f64(),
        Some(-100.0),
        "analysis refreshed"
    );
    assert!(
        got["analysis"].get("suspected_cause").is_none(),
        "wholly replaced, not merged: {}",
        got["analysis"]
    );

    // Third raise OMITTING analysis must not blank the stored reasoning.
    // Revert-check: make the assignment arm unconditional in `crates/insights/src/raise.rs` and
    // this assertion goes red.
    call(
        &node,
        &p,
        "nube",
        "insight.raise",
        raise_input("k", 3, None),
    )
    .await
    .expect("raise 3");
    let got = call(&node, &p, "nube", "insight.get", json!({ "id": &id }))
        .await
        .expect("get ok");
    assert_eq!(
        got["analysis"]["deviation"]["value"].as_f64(),
        Some(-100.0),
        "a raise with no analysis leaves the stored reasoning alone"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn analysis_and_evidence_refresh_independently() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:test", "nube", &[RAISE, GET]);

    let mut first = raise_input("k", 1, Some(analysis()));
    first["evidence"] = json!({ "source": "src-1" });
    let out = call(&node, &p, "nube", "insight.raise", first)
        .await
        .expect("raise 1");
    let id = out["id"].as_str().unwrap().to_string();

    // A producer that changes its query but not its prose: evidence supplied, analysis omitted.
    // Omission means "unchanged" for EACH field — the two lifetimes are unrelated, so neither
    // field's presence may imply anything about the other's.
    let mut second = raise_input("k", 2, None);
    second["evidence"] = json!({ "source": "src-2" });
    call(&node, &p, "nube", "insight.raise", second)
        .await
        .expect("raise 2");

    let got = call(&node, &p, "nube", "insight.get", json!({ "id": &id }))
        .await
        .expect("get ok");
    assert_eq!(got["evidence"]["source"], "src-2", "evidence refreshed");
    assert_eq!(
        got["analysis"],
        analysis(),
        "the omitted analysis is untouched by an evidence refresh"
    );
}

// --- the get-vs-list boundary -------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn list_omits_analysis_while_get_echoes_it() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:test", "nube", &[RAISE, GET, LIST]);
    let out = call(
        &node,
        &p,
        "nube",
        "insight.raise",
        raise_input("k", 1, Some(analysis())),
    )
    .await
    .expect("raise ok");
    let id = out["id"].as_str().unwrap();

    let page = call(&node, &p, "nube", "insight.list", json!({}))
        .await
        .expect("list ok");
    let item = &page["items"].as_array().unwrap()[0];
    assert!(
        item.get("analysis").is_none(),
        "list omits the reasoning (page bloat + prose disclosure): {item}"
    );
    assert_eq!(item["dedup_key"], "k", "the rest of the record is intact");

    let got = call(&node, &p, "nube", "insight.get", json!({ "id": id }))
        .await
        .expect("get ok");
    assert_eq!(
        got["analysis"]["suspected_cause"], "Meter offline or site unoccupied (weekend)",
        "get echoes it"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn list_omits_analysis_under_every_filter_and_across_a_page_boundary() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:test", "nube", &[RAISE, LIST]);
    for i in 0..3u64 {
        call(
            &node,
            &p,
            "nube",
            "insight.raise",
            raise_input(&format!("k{i}"), i + 1, Some(analysis())),
        )
        .await
        .expect("raise ok");
    }

    // A filtered, PAGED read is the shape a real roster issues — and paging is where a strip
    // applied before truncation (or on the wrong vector) would leak.
    let page = call(
        &node,
        &p,
        "nube",
        "insight.list",
        json!({ "status": "open", "severity": "warning", "limit": 2 }),
    )
    .await
    .expect("list ok");
    assert_eq!(page["items"].as_array().unwrap().len(), 2, "first page");
    assert!(
        page["next"].is_object(),
        "there is a next page: {}",
        page["next"]
    );
    assert!(
        !page.to_string().contains("suspected_cause"),
        "no reasoning on a filtered page: {page}"
    );

    let next = page["next"].clone();
    let page2 = call(
        &node,
        &p,
        "nube",
        "insight.list",
        json!({ "status": "open", "limit": 2, "cursor": next }),
    )
    .await
    .expect("list page 2");
    assert_eq!(page2["items"].as_array().unwrap().len(), 1, "second page");
    assert!(
        !page2.to_string().contains("suspected_cause"),
        "no reasoning across the page boundary either: {page2}"
    );
}

// --- the closed struct DROPS an unknown key, deliberately --------------------------------------
// This is the accepted cost of a closed vocabulary and this repo's most-repeated failure mode (a
// field added at the producer edge, silently dropped until the Rust type learns it). Pinned as a
// test so a future contributor cannot mistake the struct for an open map: the raise SUCCEEDS and
// the key is gone. `body` is the documented overflow.

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_unknown_analysis_key_is_accepted_and_dropped() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:test", "nube", &[RAISE, GET]);
    let out = call(
        &node,
        &p,
        "nube",
        "insight.raise",
        raise_input(
            "k",
            1,
            Some(json!({ "trigger_logic": "flat for 24h", "confidence": 0.9 })),
        ),
    )
    .await
    .expect("an unknown key does NOT fail the raise");
    let id = out["id"].as_str().unwrap();

    let got = call(&node, &p, "nube", "insight.get", json!({ "id": id }))
        .await
        .expect("get ok");
    assert_eq!(got["analysis"]["trigger_logic"], "flat for 24h");
    assert!(
        got["analysis"].get("confidence").is_none(),
        "the seventh field is DROPPED, not stored — use `body`: {}",
        got["analysis"]
    );
}

// --- capability-deny: analysis opens no alternate path ------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn raise_with_analysis_is_denied_without_the_raise_cap() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    // Holds every READ cap but not raise.
    let p = principal("user:mallory", "nube", &[GET, LIST]);
    let r = call(
        &node,
        &p,
        "nube",
        "insight.raise",
        raise_input("k", 1, Some(analysis())),
    )
    .await;
    assert!(
        matches!(r, Err(ToolError::Denied) | Err(ToolError::NotFound)),
        "denied opaquely, exactly as without analysis: {r:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_deny_is_identical_for_a_real_and_a_fictional_id() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let author = principal("user:test", "nube", &[RAISE]);
    let out = call(
        &node,
        &author,
        "nube",
        "insight.raise",
        raise_input("k", 1, Some(analysis())),
    )
    .await
    .expect("raise ok");
    let real_id = out["id"].as_str().unwrap().to_string();

    // Assert the property only the OUTER gate has: a reader with no `insight.get` cap must not be
    // able to tell a real id from one that exists nowhere. An inner layer failing on lookup would
    // pass the "denied" check while still leaking existence through a different error.
    let mallory = principal("user:mallory", "nube", &[LIST]);
    let real = call(
        &node,
        &mallory,
        "nube",
        "insight.get",
        json!({ "id": &real_id }),
    )
    .await;
    let fake = call(
        &node,
        &mallory,
        "nube",
        "insight.get",
        json!({ "id": "01JZZZNOPE0000000000000000" }),
    )
    .await;
    assert_eq!(
        format!("{real:?}"),
        format!("{fake:?}"),
        "a real id and a fictional id produce IDENTICAL errors"
    );
    assert!(
        !format!("{real:?}").contains("Meter offline"),
        "no prose in the error: {real:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_lister_without_get_never_receives_an_analysis_payload() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let author = principal("user:test", "nube", &[RAISE]);
    call(
        &node,
        &author,
        "nube",
        "insight.raise",
        raise_input("k", 1, Some(analysis())),
    )
    .await
    .expect("raise ok");

    // A principal holding LIST but not GET has no path to the reasoning at all.
    let reader = principal("user:bob", "nube", &[LIST]);
    let page = call(&node, &reader, "nube", "insight.list", json!({}))
        .await
        .expect("list ok");
    let dump = page.to_string();
    assert!(
        !dump.contains("Meter offline") && !dump.contains("baseline"),
        "no producer prose reaches a list-only reader: {dump}"
    );
}

// --- workspace isolation ------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn analysis_never_leaks_across_workspaces() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let a = principal("user:test", "ws-a", &[RAISE, GET, LIST]);
    let out = call(
        &node,
        &a,
        "ws-a",
        "insight.raise",
        raise_input("k", 1, Some(analysis())),
    )
    .await
    .expect("raise ok");
    let id = out["id"].as_str().unwrap();

    let b = principal("user:bob", "ws-b", &[RAISE, GET, LIST]);
    let got = call(&node, &b, "ws-b", "insight.get", json!({ "id": id })).await;
    let leaked = got.map(|v| v.to_string()).unwrap_or_default();
    assert!(
        !leaked.contains("Meter offline"),
        "ws-b cannot read ws-a's reasoning: {leaked}"
    );
    let page = call(&node, &b, "ws-b", "insight.list", json!({}))
        .await
        .expect("list ok");
    assert_eq!(
        page["items"].as_array().unwrap().len(),
        0,
        "no cross-ws rows"
    );
}

// --- the 4 KB reject ----------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn oversize_analysis_rejects_the_whole_raise_and_names_body_as_the_overflow() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:test", "ws-a", &[RAISE, LIST]);
    let big = "x".repeat(5000);
    let r = call(
        &node,
        &p,
        "ws-a",
        "insight.raise",
        raise_input("k", 1, Some(json!({ "trigger_logic": big }))),
    )
    .await;
    match &r {
        Err(ToolError::BadInput(msg)) => assert!(
            msg.contains("body"),
            "the error names `body` as the overflow — it is the producer's only teacher: {msg}"
        ),
        other => panic!("oversize analysis must reject: {other:?}"),
    }
    // Rejected UP FRONT — no orphan parent row, exactly like the occurrence + evidence caps.
    let page = call(&node, &p, "ws-a", "insight.list", json!({}))
        .await
        .expect("list ok");
    assert_eq!(page["items"].as_array().unwrap().len(), 0, "no orphan row");
}
