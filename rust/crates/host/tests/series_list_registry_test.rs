//! `series.list` lists over the **`series_meta` registry** (one row per distinct series name), NOT a
//! `GROUP BY` over the committed `series` samples table (one row per datapoint ever ingested). This
//! locks the perf fix: the old path grouped the whole samples table to recover a handful of names —
//! seconds on any real ingest volume, and `string::starts_with` in the WHERE defeated the
//! `(series, …)` indexes. The registry path is proportional to the series COUNT, not the sample count.
//!
//! The behavioural proofs a samples-scan regression would fail:
//!   - listing returns the correct **distinct**, **ascending-sorted** names;
//!   - a `prefix` filters by name;
//!   - names are **workspace-isolated** (gate 1);
//!   - and — the load-bearing one — writing MANY samples to ONE series still lists exactly ONE name.
//!     A `GROUP BY` over samples that lost its dedup, or any per-sample listing, blows this assertion;
//!     the registry (one row per name) cannot. Real store, real commit path, no mocks (rule 9).

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_ingest_tool, drain_workspace};
use lb_store::Store;
use serde_json::json;

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

/// Just the two caps this suite needs: write and list.
const CAPS: &[&str] = &["mcp:ingest.write:call", "mcp:series.list:call"];

/// Ingest `count` samples into `series` in one commit, then drain so the registry row is written.
/// The seqs run `1..=count`; the payload is the seq. This is the "many datapoints, one name" shape.
async fn write_seqs(store: &Store, p: &Principal, ws: &str, series: &str, count: u64) {
    let samples: Vec<serde_json::Value> = (1..=count)
        .map(|seq| {
            json!({
                "series": series, "producer": "x", "ts": seq, "seq": seq,
                "payload": seq, "qos": "must-deliver",
            })
        })
        .collect();
    call_ingest_tool(store, p, ws, "ingest.write", &json!({ "samples": samples }))
        .await
        .unwrap();
    drain_workspace(store, ws).await.unwrap();
}

/// `series.list(prefix)` → the returned names (prefix omitted = all).
async fn list(store: &Store, p: &Principal, ws: &str, prefix: Option<&str>) -> Vec<String> {
    let args = match prefix {
        Some(pfx) => json!({ "prefix": pfx }),
        None => json!({}),
    };
    let out = call_ingest_tool(store, p, ws, "series.list", &args)
        .await
        .unwrap();
    serde_json::from_value(out["series"].clone()).unwrap()
}

/// Distinct names, ascending — regardless of the order they were first ingested.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn lists_distinct_names_sorted() {
    let store = Store::memory().await.unwrap();
    let p = principal("prod", "acme", CAPS);
    // Ingest out of alphabetical order; several samples each.
    write_seqs(&store, &p, "acme", "gamma", 3).await;
    write_seqs(&store, &p, "acme", "alpha", 5).await;
    write_seqs(&store, &p, "acme", "beta", 4).await;

    assert_eq!(
        list(&store, &p, "acme", None).await,
        vec!["alpha", "beta", "gamma"],
        "distinct series names, ascending"
    );
}

/// A `prefix` filters by NAME (the registry read the samples-scan path could not do without the
/// index-defeating full scan).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn prefix_filters_by_name() {
    let store = Store::memory().await.unwrap();
    let p = principal("prod", "acme", CAPS);
    write_seqs(&store, &p, "acme", "sensor.temp", 2).await;
    write_seqs(&store, &p, "acme", "sensor.humidity", 2).await;
    write_seqs(&store, &p, "acme", "power.kw", 2).await;

    assert_eq!(
        list(&store, &p, "acme", Some("sensor.")).await,
        vec!["sensor.humidity", "sensor.temp"],
        "only the sensor.* names, sorted"
    );
    assert_eq!(
        list(&store, &p, "acme", Some("power")).await,
        vec!["power.kw"],
        "prefix matches the one power series"
    );
    assert!(
        list(&store, &p, "acme", Some("nope")).await.is_empty(),
        "a prefix that matches nothing lists nothing"
    );
}

/// The load-bearing regression: MANY samples in ONE series still lists exactly ONE name. A samples
/// table has `count` rows for this series; the registry has one. Listing must reflect the registry.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn many_samples_one_series_lists_one_name() {
    let store = Store::memory().await.unwrap();
    let p = principal("prod", "acme", CAPS);
    // 250 datapoints, one name. A GROUP-BY-over-samples that lost dedup — or any per-sample count —
    // would return 250 (or duplicates); the registry returns exactly one row.
    write_seqs(&store, &p, "acme", "busy", 250).await;

    assert_eq!(
        list(&store, &p, "acme", None).await,
        vec!["busy"],
        "distinct listing is by series NAME, not by sample volume"
    );

    // And writing yet more samples to the SAME series does not grow the listing — it is still one
    // name. (This is the "does not degrade with sample volume" property, asserted behaviourally.)
    write_seqs(&store, &p, "acme", "busy", 300).await;
    assert_eq!(
        list(&store, &p, "acme", None).await,
        vec!["busy"],
        "more samples in an existing series add no names"
    );
}

/// Names are workspace-isolated: each workspace lists only its own series (gate 1, the hard wall).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn names_are_workspace_isolated() {
    let store = Store::memory().await.unwrap();
    let a = principal("prod", "ws-a", CAPS);
    let b = principal("prod", "ws-b", CAPS);
    write_seqs(&store, &a, "ws-a", "temp", 3).await;
    write_seqs(&store, &b, "ws-b", "power", 3).await;

    assert_eq!(
        list(&store, &a, "ws-a", None).await,
        vec!["temp"],
        "ws-a lists only its own series"
    );
    assert_eq!(
        list(&store, &b, "ws-b", None).await,
        vec!["power"],
        "ws-b lists only its own series — no cross-workspace bleed"
    );
}
