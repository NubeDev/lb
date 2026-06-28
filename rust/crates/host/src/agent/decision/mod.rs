//! The agent **decision** — the durable first-settle gate on one suspended tool call (agent-run
//! scope Part 2). Folder-of-verbs (FILE-LAYOUT §3):
//!   - `model`  — the [`AgentDecision`] record (`agent_decision:{job}:{tool_call}`) + its id.
//!   - `store`  — `create_pending` (first-write reservation) + `flip_to_settled` (conditional bind).
//!   - `open`   — the loop's Ask action: reserve + surface inbox + transcript `SuspensionOpened` +
//!                `suspend` the job (durable-before-motion).
//!   - `settle` — the `agent.decide` core: first-settle bind + leave the job resumable.
//!
//! The decision record is **separate from `lb_inbox::Resolution` on purpose**: Resolution is
//! last-writer-wins (the coding workflow needs that); an agent Ask needs first-settle (a decided,
//! acted-on call must not flip). The Ask still surfaces an inbox item for routing, but the binding
//! settle is this record.

mod model;
mod open;
mod resume;
mod settle;
mod store;

pub use model::{decision_id, AgentDecision, DecisionState, DECISION_TABLE};
pub use open::{open_suspension, APPROVAL_CHANNEL};
pub use resume::{resume_suspensions, DENIED_BY_POLICY};
pub use settle::settle_decision;
pub use store::{load_decision, SettleOutcome};
