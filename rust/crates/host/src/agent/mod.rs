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
mod config;
mod decision;
mod defs;
mod descriptor;
mod dispatch;
mod error;
mod in_house;
mod invoke;
mod invoke_remote;
mod memory;
mod menu;
mod model_access;
mod page_context;
mod policy;
mod registry;
mod rehydrate;
mod resolve_default;
mod resolve_definition;
mod resolve_key;
mod resolve_model;
mod route;
mod run;
mod runtime;
mod runtimes;
mod serve;
mod step;
mod substrate;
mod tool;
mod unconfigured;

pub use activate::{activate_skill, Activation, SKILL_ACTIVATE};
pub use catalog::{format_catalog, render_catalog};
pub use config::{
    agent_config_get, agent_config_set, call_agent_config_tool, get_agent_config, AgentConfig,
    ModelEndpointPatch, AGENT_CONFIG_TABLE,
};
pub use decision::{
    decision_id, load_decision, settle_decision, AgentDecision, DecisionState, SettleOutcome,
    APPROVAL_CHANNEL as DECISION_APPROVAL_CHANNEL, DECISION_TABLE, DENIED_BY_POLICY,
};
pub use defs::{
    agent_def_create, agent_def_delete, agent_def_get, agent_def_list, agent_def_test,
    agent_def_update, builtin_definitions, call_agent_catalog_tool, seed_agent_definitions,
    AgentDefinition, DefinitionEndpoint, DefinitionPatch, TestContext, TestResult, AGENT_DEFS_NS,
    AGENT_DEFS_TABLE, BUILTIN_PREFIX,
};
pub use descriptor::invoke_descriptor;
pub use dispatch::{invoke_via_runtime, Substrate};
pub use error::AgentError;
pub use in_house::{InHouseRuntime, DEFAULT_RUNTIME};
pub use invoke::{invoke, resume, Invocation};
pub use invoke_remote::invoke_remote;
pub use memory::{
    call_agent_memory_tool, memory_delete, memory_get, memory_index_for_injection, memory_list,
    memory_set, render_index, Memory, MemoryKind, MemoryScope, INJECT_CAP, MAX_BODY,
    MAX_DESCRIPTION, MEMORY_HEADER,
};
pub use menu::reachable_tools;
pub use model_access::{AllowedTool, CallOutcome, ModelAccess, ProposedCall, Turn};
pub use page_context::{fence_into_goal, MAX_CONTEXT_BYTES};
pub use policy::{
    evaluate as evaluate_policy, load_policy, save_policy, ArgMatch, Effect, Policy, Rule,
    POLICY_TABLE,
};
pub use registry::RuntimeRegistry;
pub use rehydrate::{rehydrate, LoopState};
pub use resolve_default::{resolve_effective_runtime, resolve_effective_runtime_id};
pub use resolve_definition::resolve_active_definition;
pub use resolve_key::{resolve_endpoint_key, resolve_endpoint_key_host};
pub use resolve_model::{resolve_workspace_model, ModelBuilder};
pub use route::{agent_call_key, AgentInvokeReply, AgentInvokeRequest};
pub use run::{cancel_run, run_session, MAX_STEPS, SYSTEM_PROMPT};
pub use runtime::{AgentRuntime, ErasedModel, RunContext};
pub use runtimes::list_runtimes;
pub use serve::{serve_agent, AgentServer};
pub use tool::call_agent_tool;
#[allow(unused_imports)]
pub use unconfigured::{UnconfiguredModel, UNCONFIGURED_ANSWER};
