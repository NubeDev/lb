//! Rollup bucket ALIGNMENT at the HOST surface (lb#111, series-observability Decision 21).
//!
//! `crates/ingest` proves the grid arithmetic and the fold-vs-read agreement. What can only be
//! proven here is everything the MCP boundary adds:
//!   - the merge rules — absent PRESERVES, `null` CLEARS, an object SETS, and an anchor is inherited
//!     when a tier's WIDTH changes (a width is the tier's merge identity, so retuning one creates a
//!     tier the stored policy has none of — the exact shape in which the `method` bug shipped twice);
//!   - the capability gate, in BOTH directions. No new cap was minted: an anchor is a field on a
//!     policy, so it rides `mcp:series.retention.set:call`, and this file is what says that was a
//!     decision rather than an oversight;
//!   - that a bucketed READ through `series.read` resolves the governing tier's anchor by itself.
//!     That is the seam: a caller cannot be expected to know the grid its own history was folded on,
//!     so if the verb did not resolve it, every dashboard would read a mis-gridded tier and nothing
//!     would error.
//!
//! Real `Store::memory()`, real MCP dispatch through `call_ingest_tool`. No mocks.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::call_ingest_tool;
use lb_mcp::ToolError;
use lb_store::Store;
use serde_json::{json, Value};

const SET: &str = "mcp:series.retention.set:call";
const LIST: &str = "mcp:series.retention.list:call";
const READ: &str = "mcp:series.read:call";
const WRITE: &str = "mcp:ingest.write:call";
const PREFIX: &str = "plant.";
const SERIES: &str = "plant.line-1.kw";
/// 2026-07-27T00:00:00Z.
const DAY0: u64 = 1_785_110_400_000;
const MIN: u64 = 60_000;
const HOUR: u64 = 3_600_000;
const DAY: u64 = 86_400_000;
/// UTC+10's local midnight, expressed the way an operator enters it.
const LOCAL_MIDNIGHT: i64 = -10 * 3_600_000;

fn principal(sub: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: "acme".into(),
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
    call_ingest_tool(store, p, "acme", tool, &args).await
}

/// Two tiers: a 15-minute one with no anchor, and a daily one anchored at local midnight.
async fn seed_policy(store: &Store, p: &Principal) {
    call(
        store,
        p,
        "series.retention.set",
        json!({
            "prefix": PREFIX,
            "raw_for_ms": HOUR,
            "max_samples": 0,
            "tiers": [
                { "width_ms": 15 * MIN, "keep_for_ms": 7 * DAY, "method": "avg" },
                { "width_ms": DAY, "keep_for_ms": 90 * DAY, "method": "avg",
                  "align": { "origin_ms": LOCAL_MIDNIGHT } },
            ],
            "now_ms": DAY0,
        }),
    )
    .await
    .expect("seed set");
}

async fn tiers(store: &Store, p: &Principal) -> Vec<Value> {
    let out = call(store, p, "series.retention.list", json!({}))
        .await
        .expect("list");
    out["policies"]
        .as_array()
        .expect("policies is an array")
        .iter()
        .find(|r| r["prefix"] == PREFIX)
        .expect("the seeded policy exists")["tiers"]
        .as_array()
        .expect("tiers is an array")
        .clone()
}

// ------------------------------------------------------------------------------ the merge ----

/// An anchor survives the round-trip through `set` → `list`, and an unanchored tier stays unanchored
/// rather than acquiring a zero.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_anchor_round_trips_and_absence_is_not_a_zero() {
    let store = Store::memory().await.unwrap();
    let admin = principal("user:ada", &[SET, LIST]);
    seed_policy(&store, &admin).await;

    let t = tiers(&store, &admin).await;
    assert!(
        t[0].get("align").is_none(),
        "an unanchored tier must write no align key — absent and {{origin_ms:0}} are different \
         values even though they name the same grid"
    );
    assert_eq!(t[1]["align"]["origin_ms"], LOCAL_MIDNIGHT);
}

/// Absent PRESERVES. The whole point of `patch`: an editor that does not model the anchor must not
/// be able to drop it, and one that does must not have to echo every other field back.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn patching_a_tier_without_naming_its_anchor_keeps_it() {
    let store = Store::memory().await.unwrap();
    let admin = principal("user:ada", &[SET, LIST]);
    seed_policy(&store, &admin).await;

    call(
        &store,
        &admin,
        "series.retention.patch",
        json!({
            "prefix": PREFIX,
            "tiers": [
                { "width_ms": 15 * MIN, "keep_for_ms": 7 * DAY },
                { "width_ms": DAY, "keep_for_ms": 30 * DAY },
            ],
        }),
    )
    .await
    .expect("patch");

    let t = tiers(&store, &admin).await;
    assert_eq!(
        t[1]["align"]["origin_ms"], LOCAL_MIDNIGHT,
        "the anchor was dropped"
    );
    assert_eq!(t[1]["method"], "avg", "and the method with it");
    assert_eq!(
        t[1]["keep_for_ms"],
        30 * DAY,
        "the field that WAS named still changed"
    );
}

/// `null` CLEARS — the only way back to the epoch grid, and it must not be confusable with absence.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_explicit_null_clears_an_anchor() {
    let store = Store::memory().await.unwrap();
    let admin = principal("user:ada", &[SET, LIST]);
    seed_policy(&store, &admin).await;

    call(
        &store,
        &admin,
        "series.retention.patch",
        json!({ "prefix": PREFIX, "tiers": [{ "width_ms": DAY, "align": null }] }),
    )
    .await
    .expect("patch");

    let t = tiers(&store, &admin).await;
    assert_eq!(t.len(), 1, "a supplied tier list replaces the list");
    assert!(
        t[0].get("align").is_none(),
        "null must remove the key, not zero it"
    );
}

/// Retuning a tier's WIDTH inherits the anchor, exactly as it inherits the method — and for the same
/// reason: an anchor says where this series' days begin, which does not stop being true because the
/// bucket size changed. This is the shape the method bug shipped in twice.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn retuning_a_width_inherits_the_anchor() {
    let store = Store::memory().await.unwrap();
    let admin = principal("user:ada", &[SET, LIST]);
    seed_policy(&store, &admin).await;

    // 1 day → 12 hours. No stored tier has that width, so this is a NEW tier as far as the merge is
    // concerned, and it names neither method nor anchor.
    call(
        &store,
        &admin,
        "series.retention.patch",
        json!({ "prefix": PREFIX, "tiers": [{ "width_ms": 12 * HOUR, "keep_for_ms": 90 * DAY }] }),
    )
    .await
    .expect("patch");

    let t = tiers(&store, &admin).await;
    assert_eq!(t[0]["width_ms"], 12 * HOUR);
    assert_eq!(
        t[0]["method"], "avg",
        "the method is inherited (the original bug)"
    );
    assert_eq!(
        t[0]["align"]["origin_ms"], LOCAL_MIDNIGHT,
        "the anchor is inherited too — otherwise retuning a width silently re-grids the tier back \
         onto UTC midnight"
    );
}

/// A width of `0` describes no bucket and would divide by zero in the fold. Refused at the verb, so
/// no new row can carry one.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_zero_width_tier_is_refused() {
    let store = Store::memory().await.unwrap();
    let admin = principal("user:ada", &[SET, LIST]);
    let err = call(
        &store,
        &admin,
        "series.retention.set",
        json!({
            "prefix": PREFIX,
            "raw_for_ms": HOUR,
            "max_samples": 0,
            "tiers": [{ "width_ms": 0, "keep_for_ms": 0 }],
        }),
    )
    .await
    .expect_err("a zero-width tier must be refused");
    assert!(
        matches!(&err, ToolError::BadInput(m) if m.contains("width_ms")),
        "the refusal must name the field: {err:?}"
    );
}

// ------------------------------------------------------------------------------- the gate ----

/// BOTH directions. No new capability was minted for the anchor — it is a field on a policy, so it
/// rides the administrative grant that already governs writing one. A reader without that grant
/// cannot set an anchor; the same principal WITH it can.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn writing_an_anchor_needs_the_retention_set_grant() {
    let store = Store::memory().await.unwrap();
    let admin = principal("user:ada", &[SET, LIST]);
    seed_policy(&store, &admin).await;

    // A principal that may LIST policies but not set them.
    let reader = principal("user:bob", &[LIST]);
    let denied = call(
        &store,
        &reader,
        "series.retention.patch",
        json!({ "prefix": PREFIX, "tiers": [{ "width_ms": DAY, "align": { "origin_ms": 0 } }] }),
    )
    .await
    .expect_err("a caller without the set grant must be refused");
    assert!(
        matches!(denied, ToolError::Denied),
        "a refusal is opaque: {denied:?}"
    );

    // ...and the stored anchor is untouched by the attempt.
    assert_eq!(
        tiers(&store, &admin).await[1]["align"]["origin_ms"],
        LOCAL_MIDNIGHT
    );

    // The granted direction, so this test cannot pass by everything being denied.
    call(
        &store,
        &admin,
        "series.retention.patch",
        json!({ "prefix": PREFIX, "tiers": [{ "width_ms": DAY, "align": { "origin_ms": 0 } }] }),
    )
    .await
    .expect("the granted principal may set an anchor");
    assert_eq!(tiers(&store, &admin).await[0]["align"]["origin_ms"], 0);
}

// -------------------------------------------------------------------------------- the read ----

/// **The seam.** A bucketed read resolves the governing tier's anchor by itself and reports the grid
/// it used. A caller cannot know how its own history was folded, so a verb that did not do this
/// would hand every dashboard a mis-gridded tier with nothing raised.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_bucketed_read_resolves_the_governing_anchor_and_reports_it() {
    let store = Store::memory().await.unwrap();
    let admin = principal("user:ada", &[SET, LIST, READ, WRITE]);
    seed_policy(&store, &admin).await;

    // Real samples through the real write verb (which drains to the committed table).
    let samples: Vec<Value> = (0..48)
        .map(|i| {
            json!({
                "series": SERIES,
                "producer": "gw",
                "ts": DAY0 + i * HOUR,
                "seq": i,
                "payload": i as f64,
                "labels": {},
                "qos": "best-effort",
            })
        })
        .collect();
    call(
        &store,
        &admin,
        "ingest.write",
        json!({ "samples": samples }),
    )
    .await
    .expect("write");

    let out = call(
        &store,
        &admin,
        "series.read",
        json!({
            "mode": "buckets",
            "series": SERIES,
            "from": DAY0 - DAY,
            "to": DAY0 + 2 * DAY,
            "width_ms": DAY,
        }),
    )
    .await
    .expect("read");

    assert_eq!(
        out["origin_ms"], LOCAL_MIDNIGHT,
        "the read must report the grid it used, not leave the caller to guess"
    );
    for b in out["buckets"].as_array().expect("buckets") {
        let t = b["t"].as_u64().expect("t");
        assert_eq!(
            (t % DAY) / HOUR,
            14,
            "bucket {t} is not on the policy's local-midnight grid — the read floored on the epoch \
             grid instead of the tier's"
        );
    }

    // REVERT-CHECK: an explicit per-read anchor overrides the policy, and produces DIFFERENT
    // boundaries — so the assertion above could not have passed by accident.
    let epoch = call(
        &store,
        &admin,
        "series.read",
        json!({
            "mode": "buckets",
            "series": SERIES,
            "from": DAY0 - DAY,
            "to": DAY0 + 2 * DAY,
            "width_ms": DAY,
            "origin_ms": 0,
        }),
    )
    .await
    .expect("read with an explicit anchor");
    assert_eq!(epoch["origin_ms"], 0);
    for b in epoch["buckets"].as_array().expect("buckets") {
        assert_eq!(
            b["t"].as_u64().expect("t") % DAY,
            0,
            "an explicit epoch anchor was ignored"
        );
    }
}
