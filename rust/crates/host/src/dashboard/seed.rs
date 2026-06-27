//! `seed_iot_demo` — "real data without a real sensor" (dashboard scope, build step 1). Emits real
//! `Sample`s through the **real ingest path** (`lb_ingest::write` → `commit_batch`) and tags the
//! series entities over the **real tag graph** (`lb_tags::add`), so every later test and demo is
//! honest (the no-mocks/seed rule, CLAUDE §9 — this seeds real records, it does not fake them).
//!
//! It is a dev/test entrypoint, not an MCP verb — so it writes through the raw crate verbs (the same
//! ones the gated `ingest.write` + drain worker call), not the capability-gated host surface (it has
//! no principal; it is the system seeding its own workspace, like `lb_inbox::record` in the test
//! harness). The IoT-ness lives entirely in *which* series it writes and *what tags* it attaches —
//! the core never knows "cooler" or "fryer" (ingest/vision rule).

use lb_ingest::{commit_batch, write as stage_write, Qos, Sample};
use lb_store::{Store, StoreError};
use lb_tags::{add as tag_add, Provenance, Source, Tag, DEFAULT_TAG_NODE_CAP};
use serde_json::json;

/// What the seed wrote — for the integrity test to assert against.
#[derive(Debug, Clone, PartialEq)]
pub struct SeedReport {
    pub series: Vec<String>,
    pub samples_committed: usize,
}

/// The series-entity prefix the tag graph + `series.find` use (`series:<name>`).
fn entity(series: &str) -> String {
    format!("series:{series}")
}

/// Seed the `kfc`-style IoT demo into workspace `ws` at logical time base `now`: a walk-in-cooler
/// temperature series and a fryer-state series, each tagged for discovery. Returns what it wrote.
/// Idempotent on `(series, producer, seq)` — re-seeding upserts the same rows, never duplicates.
pub async fn seed_iot_demo(store: &Store, ws: &str, now: u64) -> Result<SeedReport, StoreError> {
    const PRODUCER: &str = "seed:iot-demo";
    const N: u64 = 24; // a day of hourly points — enough for a chart to look alive

    // A gently oscillating cooler temperature (°C) and a toggling fryer state (on/off).
    let mut samples = Vec::with_capacity((N * 2) as usize);
    for seq in 1..=N {
        let temp = 3.0 + ((seq as f64) * 0.7).sin() * 1.5; // ~1.5–4.5 °C
        samples.push(Sample {
            series: "cooler.temp".into(),
            producer: PRODUCER.into(),
            ts: now + seq,
            seq,
            payload: json!(round1(temp)),
            labels: json!({}),
            qos: Qos::BestEffort,
        });
        samples.push(Sample {
            series: "fryer.state".into(),
            producer: PRODUCER.into(),
            ts: now + seq,
            seq,
            payload: json!(if seq % 2 == 0 { "on" } else { "off" }),
            labels: json!({}),
            qos: Qos::BestEffort,
        });
    }

    stage_write(store, ws, &samples, 100_000).await?;
    // Drain in one pass (the batch is small) so the committed series is immediately readable.
    let pass = commit_batch(store, ws, samples.len()).await?;

    // Tag both series for faceted discovery (`series.find`). System provenance — the seed asserts it.
    let prov = Provenance::new(now, "seed:iot-demo", Source::System);
    tag(
        store,
        ws,
        "cooler.temp",
        &[
            ("kind", "temperature"),
            ("store", "downtown-0421"),
            ("equipment", "walk-in-cooler"),
        ],
        &prov,
    )
    .await?;
    tag(
        store,
        ws,
        "fryer.state",
        &[
            ("kind", "state"),
            ("store", "downtown-0421"),
            ("equipment", "fryer"),
        ],
        &prov,
    )
    .await?;

    Ok(SeedReport {
        series: vec!["cooler.temp".into(), "fryer.state".into()],
        samples_committed: pass.committed,
    })
}

/// Attach each `(key, value)` tag to `series:<name>` over the real tag graph.
async fn tag(
    store: &Store,
    ws: &str,
    series: &str,
    tags: &[(&str, &str)],
    prov: &Provenance,
) -> Result<(), StoreError> {
    let e = entity(series);
    for (k, v) in tags {
        let t = Tag::new(*k, json!(v));
        // The tag add can fail on the per-workspace tag-node cap; surface it as a store error (the
        // seed is a dev path, not a gated surface). The default cap is far above the seed's handful.
        tag_add(store, ws, &e, &t, prov, DEFAULT_TAG_NODE_CAP)
            .await
            .map_err(|e| StoreError::Decode(format!("seed tag: {e:?}")))?;
    }
    Ok(())
}

/// Round to one decimal place so the seeded payloads read cleanly.
fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}
