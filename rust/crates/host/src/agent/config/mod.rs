//! The per-workspace **agent config** surface (agent-config scope): a `workspace_agent_config:[ws]`
//! record holding the workspace's chosen default runtime + model endpoint, set by an admin and read
//! by any member. Mirrors the `prefs.set_default` pattern (an admin-settable per-workspace default
//! record) but for the operational agent choice rather than the localization axes.
//!
//! One responsibility per file (FILE-LAYOUT):
//!   - `model`  — the record/patch shapes (`AgentConfig`, `ModelEndpointPatch`), names-only.
//!   - `store`  — the SCHEMAFULL table + raw get/set (no auth; namespace-scoped composite id).
//!   - `verbs`  — the gated verbs (member `get`, admin `set` + registry validation).
//!   - `tool`   — the MCP bridge (`agent.config.*` → DTO), reached via `call_agent_tool`.

mod model;
mod store;
mod tool;
mod verbs;

pub use model::{AgentConfig, ModelEndpointPatch};
pub use store::{get_agent_config, AGENT_CONFIG_TABLE};
pub use tool::call_agent_config_tool;
pub use verbs::{agent_config_get, agent_config_set};
