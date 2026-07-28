//! **Prior-state seeders** — build the state a PREVIOUS version of the code would have left on
//! disc, so a test can start from a node with history instead of a bare host (testing-scope §2
//! category 6, §3.2 "every test starts from a bare host").
//!
//! Shared by `#[path = "support/prior_state.rs"] mod prior_state;` — integration tests are separate
//! crates, so this is the only way to share a factory between them.
//!
//! **Seeds, not mocks (rule 9).** Every builder writes REAL rows into a real embedded store
//! (`mem://`) through the same path the previous build used — `lb_ingest::set_policy` /
//! `write` + `drain_workspace` for anything today's types can still express, and a raw `UPSERT`
//! (byte-for-byte what `set_policy` issues) for a row shape today's `Policy` struct can no longer
//! produce. A seed FEEDS the real path; it never replaces it.
//!
//! The shape that motivated this file is bug #2 of
//! [#108](https://github.com/NubeDev/lb/issues/108): stale `modbus.<net>.`-style per-network policy
//! rows sitting at HIGHER precedence (longest-prefix-wins) than the newer global `modbus.` row, so a
//! changed default was correct on a fresh install and dead on every existing one. That bug needed
//! three hand-rolled JSON literals; it needs one builder now.
#![allow(dead_code)]

use lb_ingest::{Policy, Qos, Sample, Tier, RETENTION_TABLE};
use lb_store::Store;
use serde_json::{json, Value};

/// The retention-policy rows an older build left behind, as a factory (testing-scope §3 "fixtures
/// are factories"): describe the rows, then [`seed`](Self::seed) them through the real write path.
pub struct PriorRetention {
    ws: String,
    policies: Vec<Policy>,
    /// Rows whose SHAPE predates a field on today's `Policy` (e.g. a row written before
    /// `max_samples` existed). Kept as raw JSON because the typed struct can no longer emit them.
    legacy_shaped: Vec<(String, Value)>,
}

impl PriorRetention {
    /// Start describing the policy rows already on disc in `ws`.
    pub fn in_ws(ws: &str) -> Self {
        Self {
            ws: ws.to_string(),
            policies: Vec::new(),
            legacy_shaped: Vec::new(),
        }
    }

    /// The shape bug #2 was made of: the older build wrote ONE policy row **per network**, keyed
    /// `<root><net>.` — a longer prefix than the global `<root>` row a newer build writes, so it
    /// wins under longest-prefix-wins forever unless the upgrade removes it.
    ///
    /// Defaults to the old "keep everything" posture (both axes disabled), which is precisely why
    /// the newer global cap looked live and evicted nothing.
    pub fn per_network(mut self, root: &str, net: &str) -> Self {
        self.policies.push(Policy {
            prefix: format!("{root}{net}."),
            raw_for_ms: 0,
            max_samples: 0,
            tiers: vec![],
            filter: None,
            ..Default::default()
        });
        self
    }

    /// A per-network row that was genuinely operator-tuned (a horizon and/or a cap), as distinct
    /// from one that merely carries the old default. An upgrade is allowed to delete the latter and
    /// must not silently delete the former.
    pub fn tuned_per_network(
        mut self,
        root: &str,
        net: &str,
        raw_for_ms: u64,
        max_samples: u64,
    ) -> Self {
        self.policies.push(Policy {
            prefix: format!("{root}{net}."),
            raw_for_ms,
            max_samples,
            tiers: vec![],
            filter: None,
            ..Default::default()
        });
        self
    }

    /// Any policy row verbatim — the escape hatch for a shape the named builders do not cover.
    pub fn policy(mut self, policy: Policy) -> Self {
        self.policies.push(policy);
        self
    }

    /// A rollup tier on the most recently added row (the older build's downsampling config).
    pub fn with_tier(mut self, width_ms: u64, keep_for_ms: u64) -> Self {
        if let Some(p) = self.policies.last_mut() {
            p.tiers.push(Tier {
                width_ms,
                keep_for_ms,
                method: None,
                ..Default::default()
            });
        }
        self
    }

    /// A row written **before `max_samples` and `filter` existed** — the on-disc shape of a policy
    /// from an older release. Today's `Policy` always serializes `max_samples`, so this row cannot
    /// be produced by the typed setter; it is written with the same `UPSERT` statement `set_policy`
    /// issues, so what lands is exactly what the older build left.
    pub fn pre_cap_shaped(mut self, prefix: &str, raw_for_ms: u64) -> Self {
        self.legacy_shaped.push((
            prefix.to_string(),
            json!({ "prefix": prefix, "raw_for_ms": raw_for_ms, "tiers": [] }),
        ));
        self
    }

    /// Write every described row into `store`. Returns the typed rows seeded, in the order given.
    pub async fn seed(&self, store: &Store) -> Vec<Policy> {
        for policy in &self.policies {
            lb_ingest::set_policy(store, &self.ws, policy)
                .await
                .expect("seed a prior retention policy through the real write path");
        }
        for (prefix, row) in &self.legacy_shaped {
            store
                .query_ws(
                    &self.ws,
                    &format!("UPSERT type::thing('{RETENTION_TABLE}', $prefix) CONTENT $row"),
                    vec![
                        ("prefix".into(), Value::String(prefix.clone())),
                        ("row".into(), row.clone()),
                    ],
                )
                .await
                .expect("seed a prior-shaped retention row");
        }
        self.policies.clone()
    }
}

/// The committed sample history an older build left behind — the rows a changed retention default
/// has to actually govern. Seeded through the REAL ingest path (stage → drain), so the rows are
/// indistinguishable from a producer's.
pub struct PriorSeries {
    ws: String,
    series: Vec<(String, u64, u64)>,
    producer: String,
}

impl PriorSeries {
    /// Start describing the history already on disc in `ws`.
    pub fn in_ws(ws: &str) -> Self {
        Self {
            ws: ws.to_string(),
            series: Vec::new(),
            producer: "pi-7".to_string(),
        }
    }

    /// Which producer the historical rows came from (the filter/deadband anchors are per-producer).
    pub fn from_producer(mut self, producer: &str) -> Self {
        self.producer = producer.to_string();
        self
    }

    /// `count` samples on `series`, one per second from `first_ts_ms`. A real wall-clock-scale ts:
    /// the retention cutoff is on the ts axis, so epoch-zero rows would be a different (easier)
    /// test.
    pub fn history(mut self, series: &str, count: u64, first_ts_ms: u64) -> Self {
        self.series.push((series.to_string(), count, first_ts_ms));
        self
    }

    /// Stage and commit every described sample through the real write→drain path.
    pub async fn seed(&self, store: &Store) {
        for (series, count, first_ts) in &self.series {
            let samples: Vec<Sample> = (0..*count)
                .map(|i| Sample {
                    series: series.clone(),
                    producer: self.producer.clone(),
                    ts: first_ts + i * 1_000,
                    seq: i + 1,
                    payload: json!(i),
                    labels: Default::default(),
                    qos: Qos::BestEffort,
                })
                .collect();
            lb_ingest::write(store, &self.ws, &samples, 0)
                .await
                .expect("stage prior history");
        }
        lb_host::drain_workspace(store, &self.ws)
            .await
            .expect("commit prior history");
    }
}
