//! The durable run-state records (rule-chains-scope "Data (SurrealDB)"). Per-step rows so concurrent
//! step writes don't contend (the shape rubix-cube documented for its deferred `PgRunStore`):
//!   - `chain:{ws}:{id}` — the DAG (the `Chain` model from lb-rules);
//!   - `chain_run:{ws}:{run_id}` — the run lifecycle ([`ChainRunRecord`]);
//!   - `chain_step:{ws}:{run_id}:{step_id}` — per-step claim + outcome + output ([`StepStateRecord`]).
//! All workspace-walled, the one datastore — no new persistence layer.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CHAIN_TABLE: &str = "chain";
pub const CHAIN_RUN_TABLE: &str = "chain_run";
pub const CHAIN_STEP_TABLE: &str = "chain_step";

/// The CAS claim state of one step — the idempotency guard under redelivery (a lost claim no-ops).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClaimState {
    /// In-degree > 0, not yet ready.
    Pending,
    /// In-degree 0, ready to run.
    Enqueued,
    /// A worker won the claim and is running it.
    Running,
    /// Terminal (ok / failed / skipped — see `outcome`).
    Done,
}

/// The persisted run lifecycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainRunRecord {
    pub run_id: String,
    pub chain_id: String,
    /// `pending` | `success` | `partialFailure` | `failed`.
    pub status: String,
}

/// One step's durable state + recorded result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepStateRecord {
    pub run_id: String,
    pub step_id: String,
    pub claim: ClaimState,
    /// Remaining in-degree (decremented as upstreams finish).
    pub indegree: usize,
    /// `ok` | `err` | `skipped` | `` (not yet terminal).
    pub outcome: String,
    /// The step's output JSON (for downstream `${steps.x.output}` bindings). Null until ok.
    #[serde(default)]
    pub output: Value,
    /// The step's findings JSON (for `${steps.x.findings}`).
    #[serde(default)]
    pub findings: Value,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub attempts: u32,
    #[serde(default)]
    pub ms: u64,
}

/// The id of a per-step record within a run.
pub fn step_record_id(run_id: &str, step_id: &str) -> String {
    format!("{run_id}:{step_id}")
}
