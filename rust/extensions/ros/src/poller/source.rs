//! `Source` — the reusable, **driver-agnostic** seam the poll engine reads through. This is the ONE
//! place a future driver plugs its protocol in; the `Poller`, `gating`, and `Sink` never name ROS.
//!
//! A `Source` answers two questions the engine needs and nothing else:
//!   1. **What is pollable right now?** `targets()` returns the flattened set of leaf nodes with the
//!      four-level `enable` chain the engine ANDs (connection ∧ network ∧ device ∧ point). The engine
//!      does the ANDing (`gating.rs`) so the rule stays driver-agnostic and unit-tested once; the
//!      source only reports the tree + flags it sees.
//!   2. **What is a target's value?** `read(target)` fetches one leaf's current value. The engine
//!      batches the reads of the enabled set each tick and hands them to the `Sink`.
//!
//! ROS's impl is `RosSource` (`src/poller/ros_source.rs`), the only file that turns `RosApi` tree/point
//! calls into these generic shapes — the series id, the `present_value` read. Tests drive the engine
//! with a `StubSource` (no box, no gateway) to prove gating/scheduling/backoff/batching in isolation.

use async_trait::async_trait;

/// One pollable leaf, fully identified for the engine + carrying the four-level enable chain the
/// gating resolver ANDs. `series` is the fully-qualified id the `Sink` writes to (the source owns the
/// naming — ROS's is `ros.{ws}.{ros}.{net}.{dev}.{point}`), so the engine never constructs it.
///
/// The `*_enable` flags are the *reported* state of each ancestor; `gating::resolve` decides
/// pollability by ANDing them. Kept as four fields (not a pre-ANDed bool) so a gating test can assert
/// each level silences independently and the AND is exact.
#[derive(Debug, Clone, PartialEq)]
pub struct PollTarget {
    /// The driver's opaque handle for the leaf read (ROS: the point uuid). The engine passes it back
    /// to `read` verbatim; it never parses it.
    pub id: String,
    /// The fully-qualified series id this target's samples append to. Source-owned (see above).
    pub series: String,
    pub connection_enable: bool,
    pub network_enable: bool,
    pub device_enable: bool,
    pub point_enable: bool,
}

impl PollTarget {
    /// A target with all four levels enabled — the common case in a test fixture; flip a level with
    /// the `with_*` builders to exercise gating.
    pub fn enabled(id: impl Into<String>, series: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            series: series.into(),
            connection_enable: true,
            network_enable: true,
            device_enable: true,
            point_enable: true,
        }
    }
}

/// The value the engine read for one target this tick, plus the driver's own timestamp for it (ROS has
/// none per read, so the source stamps the tick's logical ts). The engine turns a batch of these into
/// `Sample`s for the `Sink`; `payload` is already the richest form (a number for a scalar point).
#[derive(Debug, Clone, PartialEq)]
pub struct Reading {
    pub series: String,
    pub value: serde_json::Value,
    pub ts: u64,
}

/// The engine's read seam. `Send + Sync` so the poll task can hold `Arc<dyn Source>` across `.await`.
#[async_trait]
pub trait Source: Send + Sync {
    /// The current pollable tree as a flat target list with each leaf's four enable flags. Called once
    /// per tick (the source may cache; ROS re-fetches the tree). An error here fails the whole tick →
    /// the engine backs off (a down box can't enumerate its tree either).
    async fn targets(&self) -> Result<Vec<PollTarget>, SourceError>;

    /// Read one target's current value. `ts` is the tick's logical timestamp the source stamps onto the
    /// `Reading` (the engine owns time; the source has none of its own per read). An error drops THIS
    /// target from the batch (one dead point doesn't sink the tick) unless it is `Unreachable`, which
    /// the engine treats as a tick-level backoff signal.
    async fn read(&self, target: &PollTarget, ts: u64) -> Result<Reading, SourceError>;
}

/// The source's error surface, split so the engine can react: `Unreachable` is the whole-box-down
/// backoff signal; `NotFound`/`Other` are per-target and merely drop that target from the batch.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum SourceError {
    #[error("source unreachable: {0}")]
    Unreachable(String),
    #[error("target not found: {0}")]
    NotFound(String),
    #[error("source error: {0}")]
    Other(String),
}
