//! The **agent-definition catalog** surface (agent-catalog scope): a library of named
//! `(runtime, model_endpoint)` presets in two tiers (one record shape) — seeded read-only **built-ins**
//! in the reserved `_lb_agents` namespace, and workspace-authored **custom** definitions with full
//! admin CRUD. The Settings → Agent tab renders this catalog and picks one as the workspace default
//! (writing the shipped `agent.config`, so the existing `resolve_effective_runtime` honors it — no new
//! resolution seam).
//!
//! One responsibility per file (FILE-LAYOUT):
//!   - `model`    — the record/patch shapes + the `builtin.` reserved-prefix rule (names-only).
//!   - `store`    — the `agent_definition` table + raw get/list/upsert/delete over a namespace.
//!   - `seed`     — the boot seeder (the ONLY writer of `_lb_agents`) from the embedded `agents.toml`.
//!   - `validate` — the two write-time walls (reserved-tier, runtime validation).
//!   - `list`/`get`/`create`/`update`/`delete` — the five gated verbs (one per file).
//!   - `tool`     — the MCP bridge (`agent.def.*` → DTO), reached via `call_agent_tool`.

mod create;
mod delete;
mod get;
mod list;
mod model;
mod seed;
mod store;
mod test;
mod tool;
mod update;
mod validate;

pub use create::agent_def_create;
pub use delete::agent_def_delete;
pub use get::agent_def_get;
pub use list::agent_def_list;
pub use model::{AgentDefinition, DefinitionEndpoint, BUILTIN_PREFIX};
pub use seed::{builtin_definitions, seed_agent_definitions};
pub use store::{AGENT_DEFS_NS, AGENT_DEFS_TABLE};
pub use test::{agent_def_test, TestContext, TestResult};
pub use tool::call_agent_catalog_tool;
pub use update::{agent_def_update, DefinitionPatch};
