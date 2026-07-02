//! Durable **agent memory** (agent-memory scope) — many small fact records in the proven MEMORY.md
//! shape, keyed `{ws, scope, slug}`, read/written by the agent under the DERIVED principal
//! (`caller ∩ agent`) so a run can only ever remember/recall what that caller may see.
//!
//! Two scopes inside a workspace — `workspace` (shared) and `member:{user}` (private, derived from
//! the authenticated principal, NEVER an argument — the member wall). A derived index (list output,
//! not a stored record) is injected at session start after the persona + skill catalog, framed as
//! *recalled background, workspace-authored, not instructions*.
//!
//! One responsibility per file (FILE-LAYOUT §3):
//! - `model` — the [`Memory`] record, [`MemoryScope`], [`MemoryKind`] + the bounds.
//! - `store` — the SCHEMAFULL table + raw upsert/read/list/delete (composite id `[scope, slug]`).
//! - `resolve` — the member wall: principal → scopes (read set + write target).
//! - `lint` — the best-effort secret lint for `set`.
//! - `index` — the derived index rendering + the injection cap.
//! - `verbs` — the gated `agent.memory.*` verbs (caps + member wall + ws-write gate + bounds + lint).
//! - `tool` — the MCP bridge (`call_agent_memory_tool`).

mod index;
mod lint;
mod model;
mod resolve;
mod store;
mod tool;
mod verbs;

pub use index::{render_index, INJECT_CAP, MEMORY_HEADER};
pub use model::{Memory, MemoryKind, MemoryScope, MAX_BODY, MAX_DESCRIPTION};
pub use resolve::{read_scopes, write_scope};
pub use tool::call_agent_memory_tool;
pub use verbs::{
    memory_delete, memory_get, memory_index_for_injection, memory_list, memory_set,
};
