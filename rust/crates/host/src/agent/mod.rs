//! The central AI **agent** service — a workspace-scoped actor that owns the tool-call loop
//! (README §6.16, agent scope). It sits beside `channel/` and `assets/` as a host service, not a
//! wasm extension, because the loop must call `caps::check` on each tool dispatch, read S4 assets
//! through the host verbs, and drive a durable job — all host-internal seams.
//!
//! The shape (one responsibility per file, FILE-LAYOUT §3):
//!   - `model_access` — the host-owned [`ModelAccess`] seam (so the host does NOT build-depend on
//!     the AI-gateway role crate; the role provides the impl). Model access only — no loop.
//!   - `authorize` — the `mcp:agent.invoke:call` gate (gate 1, on the calling node).
//!   - `substrate` — load the granted skill + read the shared doc under the DERIVED principal.
//!   - `run` — the bounded tool-call **loop** over a durable job (the agent itself).
//!   - `invoke` — the public entry: gate → substrate → loop; `resume` continues a session.
//!   - `serve` / `route` — the routed-MCP wiring: the hub answers an edge's `agent.invoke` over a
//!     Zenoh queryable (reusing the S3 routing seam), `caps::check` on the CALLING node.
//!
//! Every step re-runs `caps::check` under `agent ∩ caller` (the derived principal) — being allowed
//! to invoke the agent never implies the tools/skills/docs it may then reach (no widening).

mod activate;
mod authorize;
mod catalog;
mod decision;
mod dispatch;
mod error;
mod in_house;
mod invoke;
mod invoke_remote;
mod model_access;
mod policy;
mod registry;
mod rehydrate;
mod route;
mod run;
mod runtime;
mod serve;
mod step;
mod substrate;
mod tool;

pub use activate::{activate_skill, Activation, SKILL_ACTIVATE};
pub use catalog::{format_catalog, render_catalog};
pub use decision::{
    decision_id, load_decision, settle_decision, AgentDecision, DecisionState, SettleOutcome,
    APPROVAL_CHANNEL as DECISION_APPROVAL_CHANNEL, DECISION_TABLE, DENIED_BY_POLICY,
};
pub use dispatch::{invoke_via_runtime, Substrate};
pub use error::AgentError;
pub use in_house::{InHouseRuntime, DEFAULT_RUNTIME};
pub use invoke::{invoke, resume, Invocation};
pub use invoke_remote::invoke_remote;
pub use model_access::{AllowedTool, CallOutcome, ModelAccess, ProposedCall, Turn};
pub use policy::{
    evaluate as evaluate_policy, load_policy, save_policy, ArgMatch, Effect, Policy, Rule,
    POLICY_TABLE,
};
pub use rehydrate::{rehydrate, LoopState};
pub use registry::RuntimeRegistry;
pub use route::{agent_call_key, AgentInvokeReply, AgentInvokeRequest};
pub use runtime::{AgentRuntime, ErasedModel, RunContext};
pub use run::{cancel_run, run_session, MAX_STEPS};
pub use serve::{serve_agent, AgentServer};
pub use tool::call_agent_tool;
