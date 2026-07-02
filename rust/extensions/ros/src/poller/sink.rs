//! `Sink` — where a poll batch leaves the engine and enters the platform: it turns a tick's `Reading`s
//! into `Sample`s and calls the host's **`ingest.write`** through the `lb-sidecar-client` callback
//! (`HostCtx::client()`). Poll values are high-volume *motion* → they ride `ingest.write` (the S8
//! read-side buffer that batches/dedups), never raw pub/sub and never a per-sample store write
//! (scope: the "poll storm → write storm" failure mode to avoid).
//!
//! The `Sink` is a trait so the engine's batch shaping (one `ingest.write` per tick, the sample fields)
//! is unit-testable with a `RecordingSink` — no gateway. `IngestSink` is the real impl over the host
//! callback; it is exercised for real against a spawned gateway in the integration tests.
//!
//! **Sequence / dedup:** `ingest.write` dedups on `(series, producer, seq)`. The producer is forced to
//! the sidecar's token principal host-side (un-spoofable), so the engine only owns `seq`: a monotonic
//! per-series counter the poll task threads across ticks. Two poll cycles that read the same value get
//! distinct `seq`s (a real time-series of repeated readings), while a *retried* write of the same tick
//! reuses its `seq` (idempotent). The engine passes the seq in; the sink does not invent it.

use async_trait::async_trait;
use serde_json::json;

use super::source::Reading;
use crate::host::{HostCtx, HostError};

/// One sample the engine hands the sink: a reading plus its dedup `seq`. Kept separate from `Reading`
/// (which the `Source` produces) so the source stays unaware of the ingest sequence contract.
#[derive(Debug, Clone, PartialEq)]
pub struct SampleOut {
    pub series: String,
    pub value: serde_json::Value,
    pub ts: u64,
    pub seq: u64,
}

impl SampleOut {
    pub fn from_reading(r: &Reading, seq: u64) -> Self {
        Self {
            series: r.series.clone(),
            value: r.value.clone(),
            ts: r.ts,
            seq,
        }
    }
}

/// The engine's write seam. One call per tick with the whole enabled batch (`ingest.write` takes a
/// `Sample[]`), so N points cost one host round-trip, not N.
#[async_trait]
pub trait Sink: Send + Sync {
    /// Commit a tick's batch. An empty batch is a no-op (no ingest call — nothing to write).
    async fn write(&self, batch: &[SampleOut]) -> Result<(), HostError>;
}

/// The real sink: one `ingest.write` MCP callback per tick. The producer is stamped host-side from the
/// sidecar's ws-scoped token, so this never sets it (and could not spoof it if it tried).
pub struct IngestSink {
    host: HostCtx,
}

impl IngestSink {
    pub fn new(host: HostCtx) -> Self {
        Self { host }
    }
}

#[async_trait]
impl Sink for IngestSink {
    async fn write(&self, batch: &[SampleOut]) -> Result<(), HostError> {
        if batch.is_empty() {
            return Ok(());
        }
        let samples: Vec<serde_json::Value> = batch
            .iter()
            .map(|s| {
                json!({
                    "series": s.series,
                    // Producer is overridden host-side from the token; a placeholder keeps the field
                    // present for the `Sample` deserialize without asserting an identity.
                    "producer": "",
                    "ts": s.ts,
                    "seq": s.seq,
                    "payload": s.value,
                })
            })
            .collect();
        self.host
            .client()
            .call_tool("ingest.write", json!({ "samples": samples }))
            .await?;
        Ok(())
    }
}
