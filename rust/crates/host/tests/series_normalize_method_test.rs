//! The tier-METHOD half of the series-normalize MCP surface: the governing tier's method becomes the
//! bucket `value` column, longest-prefix-wins picks it, a per-read argument overrides it, the
//! resolved method is reported back, it survives a ZOOM to any width, and an unknown name is a
//! `BadInput` rather than a guess or a denial.
//!
//! The filter/policy half is `series_normalize_test.rs`; the mandatory deny + isolation categories
//! are `series_normalize_caps_test.rs`. Real node boot, real store, real MCP dispatch — no mocks.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_ingest_tool, drain_workspace, Node};
use lb_ingest::{Qos, Sample};
use lb_mcp::ToolError;
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
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

/// An admin principal for `ws` — every series cap this suite touches.
fn admin(ws: &str) -> Principal {
    principal(
        "user:test",
        ws,
        &[
            "mcp:series.retention.set:call",
            "mcp:series.retention.list:call",
            "mcp:series.retention.gc:call",
            "mcp:series.read:call",
            "mcp:series.latest:call",
            "mcp:ingest.write:call",
        ],
    )
}

async fn call(
    node: &Node,
    p: &Principal,
    ws: &str,
    tool: &str,
    args: Value,
) -> Result<Value, ToolError> {
    call_ingest_tool(&node.store, p, ws, tool, &args).await
}

/// Commit `samples` through the real write → drain path.
async fn seed(node: &Node, ws: &str, samples: Vec<Sample>) {
    lb_ingest::write(&node.store, ws, &samples, 0)
        .await
        .unwrap();
    drain_workspace(&node.store, ws).await.unwrap();
}

fn sample_at(series: &str, producer: &str, seq: u64, ts: u64, payload: Value) -> Sample {
    Sample {
        series: series.into(),
        producer: producer.into(),
        ts,
        seq,
        payload,
        labels: json!({}),
        qos: Qos::BestEffort,
    }
}

// ---------------------------------------------------------------- mandatory: capability deny ----

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_governing_tiers_method_becomes_the_bucket_value_column() {
    let node = Node::boot().await.unwrap();
    let p = admin("nube");

    call(
        &node,
        &p,
        "nube",
        "series.retention.set",
        json!({
            "prefix": "plant.",
            "raw_for_ms": 0,
            "tiers": [{"width_ms": 10000, "keep_for_ms": 0, "method": "avg"}]
        }),
    )
    .await
    .unwrap();
    // A state series on its own LONGER prefix reads as a step chart — longest-prefix-wins.
    call(
        &node,
        &p,
        "nube",
        "series.retention.set",
        json!({
            "prefix": "plant.coil",
            "raw_for_ms": 0,
            "tiers": [{"width_ms": 10000, "keep_for_ms": 0, "method": "last"}]
        }),
    )
    .await
    .unwrap();

    seed(
        &node,
        "nube",
        vec![
            sample_at("plant.temp", "p", 1, 1_000, json!(10.0)),
            sample_at("plant.temp", "p", 2, 2_000, json!(20.0)),
            sample_at("plant.coil", "p", 1, 1_000, json!(0)),
            sample_at("plant.coil", "p", 2, 2_000, json!(0)),
            sample_at("plant.coil", "p", 3, 3_000, json!(1)),
        ],
    )
    .await;

    let read = |series: &'static str, extra: Value| {
        let mut args =
            json!({"series": series, "mode": "buckets", "from": 0, "to": 10000, "width_ms": 10000});
        if let Some(o) = extra.as_object() {
            for (k, v) in o {
                args[k] = v.clone();
            }
        }
        args
    };

    let temp = call(
        &node,
        &p,
        "nube",
        "series.read",
        read("plant.temp", json!({})),
    )
    .await
    .unwrap();
    assert_eq!(
        temp["method"],
        json!("avg"),
        "the resolved method is reported back"
    );
    assert_eq!(temp["buckets"][0]["value"], json!(15.0));

    let coil = call(
        &node,
        &p,
        "nube",
        "series.read",
        read("plant.coil", json!({})),
    )
    .await
    .unwrap();
    assert_eq!(coil["method"], json!("last"), "the LONGER prefix governs");
    assert_eq!(coil["buckets"][0]["value"], json!(1));

    // An explicit per-read override beats the tier.
    let overridden = call(
        &node,
        &p,
        "nube",
        "series.read",
        read("plant.temp", json!({"method": "max"})),
    )
    .await
    .unwrap();
    assert_eq!(overridden["method"], json!("max"));
    assert_eq!(overridden["buckets"][0]["value"], json!(20.0));

    // The full stat row is still on the wire — the method ADDS a column, it removes nothing.
    let b = &temp["buckets"][0];
    for key in ["t", "min", "max", "avg", "last", "count", "first"] {
        assert!(b.get(key).is_some(), "bucket lost `{key}`: {b}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_unknown_method_is_bad_input_not_a_denial_and_not_a_guess() {
    let node = Node::boot().await.unwrap();
    let err = call(
        &node,
        &admin("nube"),
        "nube",
        "series.read",
        json!({"series": "m.v", "mode": "buckets", "from": 0, "to": 10000, "width_ms": 10000, "method": "p95"}),
    )
    .await
    .unwrap_err();
    match err {
        ToolError::BadInput(m) => {
            assert!(m.contains("p95"), "{m}");
            assert!(m.contains("nearest"), "the error names the closed set: {m}");
        }
        other => panic!("expected BadInput (author feedback), got {other:?} — a 403 would read as a capability denial"),
    }
}

/// A method must survive a ZOOM. It describes how a series reads, not how one tier is stored — so a
/// coil configured `last` reads as a step chart at every width, not only at the tier's own. Before
/// this, a 60 s read of a 900 s `avg` tier resolved NO method and the caller silently fell back to
/// averaging (found live on a real node).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_tiers_method_still_governs_a_read_at_a_different_width() {
    let node = Node::boot().await.unwrap();
    let p = admin("nube");
    call(
        &node,
        &p,
        "nube",
        "series.retention.set",
        json!({"prefix": "z.", "raw_for_ms": 0,
               "tiers": [{"width_ms": 900000, "keep_for_ms": 0, "method": "last"}]}),
    )
    .await
    .unwrap();
    seed(
        &node,
        "nube",
        vec![
            sample_at("z.coil", "p", 1, 1_000, json!(0)),
            sample_at("z.coil", "p", 2, 2_000, json!(1)),
        ],
    )
    .await;

    for width in [900_000u64, 60_000, 10_000] {
        let out = call(
            &node,
            &p,
            "nube",
            "series.read",
            json!({"series": "z.coil", "mode": "buckets", "from": 0, "to": 900000, "width_ms": width}),
        )
        .await
        .unwrap();
        assert_eq!(
            out["method"],
            json!("last"),
            "width {width} lost the method: {out}"
        );
    }
}
