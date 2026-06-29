//! The store layer — the two preference records over SurrealDB (prefs scope, all **state**, no bus).
//! One verb per file; namespace-scoped (the workspace wall). These are RAW verbs: the host service
//! is the capability chokepoint and calls them after `caps::check`. No authorization lives here.

mod default_get;
mod default_set;
mod get;
mod resolve_chain;
mod schema;
mod set;

pub use default_get::get_workspace_prefs;
pub use default_set::set_workspace_prefs;
pub use get::get_user_prefs;
pub use resolve_chain::resolve_chain;
pub use schema::{define_prefs_schema, USER_PREFS_TABLE, WORKSPACE_PREFS_TABLE};
pub use set::set_user_prefs;
