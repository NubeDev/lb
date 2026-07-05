//! The **persona** surface (agent-personas topic, sub-scope #1 — the foundation). A persona is a
//! `{ granted_tools, grounding_skills, identity, extends }` bundle of already-shipped grant-gated data,
//! selected per workspace and applied at run assembly to **narrow** a run (advertised tools + pinned
//! skills + identity) — never to widen the capability wall. Two tiers, one shape (the
//! `agent_definition` catalog pattern, fourth reuse): seeded read-only **built-ins** in the reserved
//! `_lb_personas` namespace, and workspace-authored **custom** personas with admin CRUD.
//!
//! One responsibility per file (FILE-LAYOUT):
//!   - `model`    — the `Persona` record + `PolicyPreset` + the `builtin.` reserved-prefix rule.
//!   - `store`    — the `persona` table + raw get/list/upsert/delete over a namespace (schemaless).
//!   - `seed`     — the boot seeder (the ONLY writer of `_lb_personas`) from the embedded `personas.toml`.
//!   - `validate` — the write-time walls (reserved-tier, glob grammar, `extends` cycle/depth).
//!   - `list`/`get`/`create`/`update`/`delete` — the five gated verbs (one per file).
//!   - `tool`     — the MCP bridge (`agent.persona.*` → DTO), reached via `call_agent_tool`.
//!   - `resolve`  — "which persona is active" + the `extends`-closure union (the resolve-at-read seam).
//!   - `apply`    — the ONE run-assembly filter both runtimes call: narrow the menu, fold identity +
//!                  pinned-skill bodies into the goal (fail-closed), enforce the runtime restriction.

mod apply;
mod create;
mod delete;
mod get;
mod list;
mod model;
mod resolve;
mod seed;
mod store;
mod tool;
mod update;
mod validate;

pub use apply::{
    apply_policy_preset, build_identity_fold, check_runtime, glob_matches, narrow_tools,
};
pub use create::agent_persona_create;
pub use delete::agent_persona_delete;
pub use get::agent_persona_get;
pub use list::agent_persona_list;
pub use model::{Persona, PolicyPreset};
pub use resolve::{resolve_effective, resolve_persona, EffectivePersona};
pub use seed::{builtin_personas, seed_personas};
pub use store::{PERSONA_NS, PERSONA_TABLE};
pub use tool::call_agent_persona_tool;
pub use update::{agent_persona_update, PersonaPatch};
