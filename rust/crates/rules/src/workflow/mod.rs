//! The chain DAG primitives — the pure model + binding/result logic, **lifted from rubix-cube's
//! `workflow/`** (MIT/Apache-2.0). The durable coordinator/run-store is NOT here: it lives in the host
//! over `lb-jobs` + SurrealDB (rule-chains-scope: "keep rubix-cube's trait shape, implement over our
//! store"). This module is engine-agnostic DAG math + value passing only.

mod context;
mod model;

pub use context::{ChainResult, ChainStatus, Outcome, RunContext, StepRecord, StepResult};
pub use model::{Chain, DagError, FailurePolicy, RetrySpec, Step, StepId, Trigger};
