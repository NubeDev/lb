//! The durable run-state records (flow-run-scope "Data (SurrealDB)"), mirroring the chain run-store
//! shape (Decision 6: one engine, `chains.*` the alias). Per-node rows so concurrent branch jobs
//! don't contend. All workspace-walled, the one datastore — no new persistence layer.
//!
//! - `flow:{ws}:{id}` — the typed graph (the `lb_flows::Flow` model);
//! - `flow_run:{ws}:{run_id}` — the run coordinator: lifecycle + the **pinned `flow_version`**
//!   (Decision 1) + the run params;
//! - `flow_step_output:{ws}:{run_id}:{node_id}` — per-node CAS claim (`Enqueued→Running`, the
//!   cross-node exactly-once owner, Decision 8) + outcome + output/findings;
//! - `flow_node_state:{ws}:{flow}:{node}` — last-value (Decision 5, the dashboard instant read);
//! - `flow_input:{ws}:{flow}:{node}` — retained inject values (Decision 9, read by every run).

use lb_flows::table;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The CAS claim state of one node — the idempotency guard under redelivery (a lost claim no-ops).
/// Identical to the chain `ClaimState` (Decision 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClaimState {
    Pending,
    Enqueued,
    Running,
    Done,
}

/// The persisted run coordinator: lifecycle + the **pinned `flow_version`** (Decision 1) + params.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowRunRecord {
    pub run_id: String,
    pub flow_id: String,
    /// The flow version this run pinned at start (Decision 1) — a live run is immune to edits.
    pub flow_version: u32,
    /// `pending` | `success` | `partialFailure` | `failed` | `suspended` | `cancelled`.
    pub status: String,
    #[serde(default)]
    pub params: Value,
    #[serde(default)]
    pub ts: u64,
}

/// One node's durable state + recorded result (mirrors the chain `StepStateRecord`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowStepRecord {
    pub run_id: String,
    pub node_id: String,
    pub claim: ClaimState,
    pub indegree: usize,
    /// `ok` | `err` | `skipped` | `` (not yet terminal).
    pub outcome: String,
    #[serde(default)]
    pub output: Value,
    #[serde(default)]
    pub findings: Value,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub attempts: u32,
    #[serde(default)]
    pub ms: u64,
}

/// The id of a per-node record within a run.
pub fn step_record_id(run_id: &str, node_id: &str) -> String {
    format!("{run_id}:{node_id}")
}

/// Re-export the table constants from `lb-flows` so the host verbs agree on names.
pub use lb_flows::table as tables;
pub const FLOW_TABLE: &str = table::FLOW;
pub const FLOW_RUN_TABLE: &str = table::FLOW_RUN;
pub const FLOW_STEP_TABLE: &str = table::FLOW_STEP;
pub const FLOW_NODE_STATE_TABLE: &str = table::FLOW_NODE_STATE;
pub const FLOW_INPUT_TABLE: &str = table::FLOW_INPUT;
