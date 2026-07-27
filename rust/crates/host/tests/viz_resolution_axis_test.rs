//! **The ZOOM axis at the dashboard layer.** `viz.query` derives a bucket width from
//! `(range, budget, minInterval)` and injects it into a mode-less `series.read`; `series.read` then
//! resolves the governing tier's `method` for that width. Both are width-keyed, and
//! `viz_resolution_test.rs` exercises each at a handful of individual points.
//!
//! One point is not the axis. Bug #3 of [#108] was exactly this: `method` resolved only at a tier's
//! EXACT `width_ms`, so a coil configured `last` drew as a step chart at 15 min and silently
//! averaged the instant a dashboard zoomed. Every test passed, because every test asked at the
//! configured width. `docs/scope/testing/testing-scope.md` §3.2, row 3.
//!
//! So this file sweeps:
//!   - every zoom level × budget × `minInterval` through the real `viz.query`, asserting the
//!     derived width is one the bucket ENGINE accepts and the budget holds — the two `MAX_BUCKETS`
//!     constants (`viz::resolution`'s local mirror and `lb_ingest::bucket`'s) agree at every point
//!     on the axis, not just where someone happened to look;
//!   - every step of the resolution LADDER through the real `series.read`, asserting the configured
//!     method governs and each bucket carries its `value` at all 17 widths a panel can derive.
//!
//! Real node boot, real store, real ingest + MCP dispatch — no mocks (testing §0).

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, Node};
use serde_json::{json, Value};

const VIZ: &str = "mcp:viz.query:call";
const READ: &str = "mcp:series.read:call";
const WRITE: &str = "mcp:ingest.write:call";
const SET: &str = "mcp:series.retention.set:call";

/// The panel-resolution ladder, verbatim from `host/src/viz/resolution.rs` — every width a panel
/// can derive below the 30 d top step. Duplicated here on purpose: the module is private, and a
/// test that imported the same constant could not notice the two drifting.
const LADDER_MS: &[u64] = &[
    1_000,
    5_000,
    10_000,
    30_000,
    60_000,
    300_000,
    600_000,
    900_000,
    1_800_000,
    3_600_000,
    7_200_000,
    10_800_000,
    21_600_000,
    43_200_000,
    86_400_000,
    604_800_000,
    2_592_000_000,
];

/// The window the seeded data lives in; every read below covers it from 0.
const WINDOW_MS: u64 = 2_000_000;
const SEEDED: u64 = 2_000;

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

async fn call(node: &Arc<Node>, p: &Principal, ws: &str, tool: &str, args: Value) -> Value {
    let out = call_tool(node, p, ws, tool, &args.to_string())
        .await
        .unwrap_or_else(|e| panic!("{tool} failed: {e:?}"));
    serde_json::from_str(&out).expect("json reply")
}

/// Seed a step-shaped series (alternating 0/1, the coil an operator sets `last` for) through the
/// real `ingest.write` verb, in chunks so each write drains its own batch.
async fn seed_coil(node: &Arc<Node>, p: &Principal, ws: &str, series: &str) {
    let mut i = 1u64;
    while i <= SEEDED {
        let end = (i + 499).min(SEEDED);
        let samples: Vec<Value> = (i..=end)
            .map(|seq| {
                json!({ "series": series, "producer": "seed", "ts": seq * 1000, "seq": seq,
                        "payload": (seq / 100) % 2, "qos": "best-effort" })
            })
            .collect();
        call(node, p, ws, "ingest.write", json!({ "samples": samples })).await;
        i = end + 1;
    }
}

/// EVERY zoom level a panel can be at must produce a width the bucket engine ACCEPTS, under every
/// budget and every `minInterval`. `viz::resolution` clamps to its own local `MAX_BUCKETS` mirror
/// while `lb_ingest::effective_width` REJECTS an over-cap width outright — so the moment those two
/// constants disagree, some region of the axis returns an empty panel with no error anyone reads.
///
/// A single-range test cannot see that: it samples one point of a three-dimensional space.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn every_zoom_level_yields_a_width_the_engine_accepts() {
    let ws = "viz-axis-zoom";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal("user:ada", ws, &[VIZ, READ, WRITE]);
    seed_coil(&node, &p, ws, "cpu").await;

    // Zoom levels from a one-minute live view out to two years — the panel is at `from = 0` in all
    // of them, so the seeded window is always inside the range and a non-empty frame is the honest
    // signal that the read succeeded rather than erroring into an empty one.
    const H: u64 = 3_600_000;
    const D: u64 = 86_400_000;
    let ranges = [60_000u64, H, 6 * H, D, 7 * D, 30 * D, 365 * D, 2 * 365 * D];
    // The extremes of the budget axis (a tight panel and the engine's own cap) and the interesting
    // `minInterval` cases: none, a floor below the derived width, and one above it.
    let budgets = [100u64, 2_000];
    let min_intervals = ["", "15m", "1h"];

    for &range in &ranges {
        for &budget in &budgets {
            for mi in min_intervals {
                let panel = json!({
                    "sources": [{ "refId": "A", "tool": "series.read",
                                  "args": { "series": "cpu", "from": 0u64, "to": range } }],
                    "queryOptions": { "maxDataPoints": budget, "minInterval": mi },
                });
                let out = call(
                    &node,
                    &p,
                    ws,
                    "viz.query",
                    json!({ "panel": panel, "now": 1 }),
                )
                .await;
                let rows = out["rows"].as_array().cloned().unwrap_or_default();
                assert!(
                    !rows.is_empty(),
                    "range={range} budget={budget} minInterval={mi:?}: the panel came back EMPTY — \
                     the derived width is one the bucket engine rejected (the two MAX_BUCKETS \
                     constants have drifted), and nothing on screen says so"
                );
                assert!(
                    rows.len() as u64 <= budget,
                    "range={range} budget={budget} minInterval={mi:?}: {} buckets exceeds the \
                     budget — a ceiling that only holds at the tested range is not a ceiling",
                    rows.len()
                );
                assert!(
                    rows[0].get("t").is_some(),
                    "range={range}: the bucket shape survived to the frame rows: {}",
                    rows[0]
                );
            }
        }
    }
}

/// THE REGRESSION SHAPE, at the seam a dashboard actually calls. A tier's `method` describes how the
/// SERIES reads, so a coil configured `last` must read as a step chart at EVERY width a panel can
/// derive — all seventeen ladder steps, not the one that happens to equal the tier's `width_ms`.
///
/// Before the fix, a 60 s read of a 900 s `avg` tier resolved NO method and the caller fell back to
/// averaging. Asserting at 900 s alone could never have said so.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_configured_method_governs_at_every_ladder_width() {
    let ws = "viz-axis-method";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal("user:ada", ws, &[VIZ, READ, WRITE, SET]);
    call(
        &node,
        &p,
        ws,
        "series.retention.set",
        json!({ "prefix": "plant.coil", "raw_for_ms": 0,
                "tiers": [{ "width_ms": 900_000, "keep_for_ms": 0, "method": "last" }] }),
    )
    .await;
    seed_coil(&node, &p, ws, "plant.coil").await;

    for &width in LADDER_MS {
        // Keep the read inside the engine's bucket cap by widening the window with the step: the
        // axis under test is the WIDTH, not the window.
        let to = WINDOW_MS.max(width);
        let out = call(
            &node,
            &p,
            ws,
            "series.read",
            json!({ "series": "plant.coil", "mode": "buckets",
                    "from": 0u64, "to": to, "width_ms": width }),
        )
        .await;
        assert_eq!(
            out["method"],
            json!("last"),
            "width {width} lost the method — a coil configured `last` silently averages at this \
             zoom: {out}"
        );
        let buckets = out["buckets"].as_array().expect("buckets");
        assert!(!buckets.is_empty(), "width {width} returned no buckets");
        assert!(
            buckets.iter().all(|b| b.get("value").is_some()),
            "width {width}: a bucket carries no `value` column despite a governing method"
        );
        // The step chart's values stay members of the coil's own domain — never an average of them.
        assert!(
            buckets
                .iter()
                .all(|b| b["value"] == json!(0) || b["value"] == json!(1)),
            "width {width}: `last` produced a value outside the coil's {{0,1}} domain — it was \
             aggregated, not sampled: {buckets:?}"
        );
    }
}
