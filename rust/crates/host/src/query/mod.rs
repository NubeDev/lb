//! The **query** host service (query scope) — saved PRQL/raw queries, one authoring language across
//! every source. It owns the `query:{ws}:{id}` record and the compile→dispatch pipeline, one
//! responsibility per file (FILE-LAYOUT §3):
//!   - `record`        — the `query:{ws}:{id}` store record (lang + text + target + params).
//!   - `authorize`     — the `mcp:query.<verb>:call` gate (workspace-first).
//!   - `target`        — parse `target` → engine + the no-widening underlying-cap map.
//!   - `materialize`   — compile `text` to the target's SQL (PRQL via `lb-prql`, or `raw` verbatim).
//!   - `save`/`get`/`list`/`delete` — the CRUD verbs over the record.
//!   - `compile_verb`  — `query.compile` (pure dry-run → `{sql}`).
//!   - `run`           — `query.run` (compile → no-widening target cap → dispatch to `store.query` /
//!     `federation.query`).
//!   - `descriptors`   — the `tools.catalog` descriptors for the editor verbs.
//!   - `tool`          — the `query.*` MCP bridge dispatch.
//!
//! PRQL is the authoring layer only — SurrealDB stays the one datastore (rule 2); external DBs stay
//! federated sources behind the gated `federation` extension. `query.run` **composes** the target's
//! existing capability, it never widens it (rule 5).

mod authorize;
mod compile_verb;
mod delete;
mod descriptors;
mod error;
mod get;
mod materialize;
mod record;
mod run;
mod save;
mod target;
mod tool;

pub use compile_verb::query_compile;
pub use delete::query_delete;
pub use descriptors::{compile_descriptor, run_descriptor, save_descriptor};
pub use error::QueryError;
pub use get::{query_get, query_list, QuerySummary};
pub use record::{query_tag, resolve as resolve_query, SavedQuery, TABLE};
pub use run::{query_run, RunSource};
pub use save::query_save;
pub use target::QueryTarget;
pub use tool::call_query_tool;
