//! The canonical `Sample` envelope every producer — internal or external — normalizes to
//! (ingest scope). Generic and domain-free: there is no "device", "sensor", or "metric" here, a
//! sample is just a timestamped, sequenced value in a named `series`. IoT is one *caller* of this
//! shape (via an out-of-core protocol-bridge extension), never a concept in the crate.
//!
//! The dedup identity is **`(series, producer, seq)`** — NOT `(series, seq)`. A series may have
//! many producers (the opening use case is "a fleet of edge nodes reporting state"), so keying on
//! `(series, seq)` would let producer-B's seq=5 silently upsert over producer-A's — data loss
//! disguised as idempotency. `producer` is the **authenticated calling principal** (un-spoofable,
//! already workspace-scoped); `seq` is monotonic per `(series, producer)`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Delivery guarantee for a series. `MustDeliver` carries the "never lost until on disk" promise
/// (producer keeps a durable staging copy, pruned only after a commit ack; overflow → dead-letter).
/// `BestEffort` is lossy by design (high-rate telemetry): overflow → drop-oldest, no per-sample ack,
/// and the "never lost" promise does NOT apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Qos {
    BestEffort,
    MustDeliver,
}

impl Default for Qos {
    fn default() -> Self {
        Qos::BestEffort
    }
}

/// One data point. `payload` is **any SurrealDB-typed value** (scalar, nested object, array, or a
/// record-as-content reference for binary — buckets are unavailable on the embedded engine per the
/// store spike). The buffer is payload-agnostic; only the commit step picks the storage shape.
/// `labels` are the producer's *raw wire declaration* — at commit they convert to tag-graph edges
/// once per series (the tag service is the single source of truth, not these inline labels).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sample {
    /// The named, workspace-scoped sequence this point belongs to (e.g. `node.cpu_temp`).
    pub series: String,
    /// The producing stream id — half of the dedup identity, and always ROOTED at the authenticated
    /// principal. A caller MAY declare a sub-namespace here (e.g. a per-process epoch); `ingest.write`
    /// rewrites this to `{principal}` or `{principal}/{declared}`, so the root is un-spoofable while
    /// one principal can still run many independent streams.
    ///
    /// Declare a sub-namespace whenever your `seq` counter can restart (it lives in memory), or the
    /// new stream re-enters the old one's seq space and `series.latest` — which returns the highest
    /// `seq` — pins to a pre-restart sample.
    pub producer: String,
    /// A caller-supplied logical timestamp (datetime). Data, never the ordering key — an external
    /// producer's clock is untrusted and may skew.
    pub ts: u64,
    /// Monotonic sequence per `(series, producer)` — the ordering + dedup key.
    pub seq: u64,
    /// The typed value. Stored in its richest form at commit (scalar→number/bool, structured→nested
    /// object, binary→record-as-content), never an opaque JSON blob.
    pub payload: Value,
    /// Producer's raw dimension declaration (`{host: "pi-7"}`). Converted to tag edges at commit.
    #[serde(default)]
    pub labels: Value,
    /// The delivery guarantee for this sample's series.
    #[serde(default)]
    pub qos: Qos,
}

impl Sample {
    /// The record id for a sample: the composite `[series, producer, seq]`. This is what makes the
    /// commit an idempotent UPSERT (a re-delivered sample upserts the same row, never a duplicate)
    /// and a per-producer time range a fast ID-range scan.
    pub fn record_id(&self) -> [Value; 3] {
        [
            Value::String(self.series.clone()),
            Value::String(self.producer.clone()),
            Value::Number(self.seq.into()),
        ]
    }
}
