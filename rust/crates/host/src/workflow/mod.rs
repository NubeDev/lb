//! The **coding workflow** service — the S6 worked example end to end (vision
//! `0002-coding-agent-workplace.md`, coding-workflow scope). It sits beside `agent/`, `channel/`,
//! and `assets/` as a host service (not a wasm extension) because the orchestration must drive
//! `caps::check`, the S5 agent loop, durable jobs, and the transactional outbox — all host-internal
//! seams (same reasoning as the agent being a host service).
//!
//! It holds **no durable state** (stateless extensions, §3.4): every fact lives in a record — the
//! issue + approval in the inbox, the conversation in the agent's job, the effects in the outbox.
//! Kill it mid-flight and another invocation resumes from those records.
//!
//! The flow, one responsibility per file (FILE-LAYOUT §3):
//!   - `ingest_issue`     — github-bridge writes an inbox `needs:triage` item.
//!   - `ingest_via_bridge`— compose the installed `github-bridge` wasm `normalize` tool with
//!     `ingest_issue` (S7: the bridge is a sandboxed transform artifact; the host owns the write).
//!   - `triage`           — drive the S5 agent to draft + share a scope doc (vision steps 2–4).
//!   - `request_approval` — write the `needs:approval` inbox item (the gate's subject).
//!   - `resolve_approval` — a reviewer's resolution (approve/reject/defer).
//!   - `start_coding_job` — THE GATE: start the durable job only on `Approved`; effects via outbox.
//!   - `emit_effect`      — the transactional must-deliver write (job step + outbox row, one tx).
//!   - `relay_outbox`     — deliver pending effects at-least-once with retry, through a `Target`.
//!   - `tool`             — the `workflow.*` MCP bridge (the store/orchestration verbs).
//!
//! Every external effect goes through the outbox (never raw pub/sub); progress chatter rides the bus
//! as fire-and-forget motion. The two message classes are kept distinct (§6.2).

mod authorize;
mod effect;
mod error;
mod ingest;
mod ingest_via_bridge;
mod relay;
mod request_approval;
mod resolve_approval;
mod start_job;
mod target;
mod tool;
mod triage;

pub use effect::emit_effect;
pub use error::WorkflowError;
pub use ingest::{ingest_issue, TRIAGE_CHANNEL};
pub use ingest_via_bridge::ingest_via_bridge;
pub use relay::{relay_outbox, RelayPass};
pub use request_approval::{request_approval, APPROVAL_CHANNEL};
pub use resolve_approval::resolve_approval;
pub use start_job::{start_coding_job, CodingJob};
pub use target::Target;
pub use tool::call_workflow_tool;
pub use triage::{triage, Triaged};
