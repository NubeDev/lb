//! The **workspaces** service — list / create the workspaces a session may see (collaboration scope,
//! slice 2). Workspaces are SurrealDB namespaces (the hard wall §7); this service adds a thin durable
//! **directory** of them (in a reserved namespace, like the workflow directory) so the UI can show a
//! workspace list / switcher / create instead of the hardcoded `acme`.
//!
//! The directory is **node-level operator config** (which workspaces exist on this node), not a
//! tenant's own data — so it lives in a reserved namespace ([`WORKSPACES_NS`]), the one deliberate
//! exception to "every key is workspace-scoped" (§7 carves out node infrastructure; the same reasoning
//! as the workflow directory). `workspace_create` registers one; `workspace_list` enumerates them.
//!
//! Authorization is the MCP gate (`mcp:workspace.<verb>:call`) through `authorize_tool`. Note the
//! workspace passed to the gate is the *session's* workspace (from the token) — a principal must hold
//! the verb in its own workspace to read/extend the directory. One verb per file (FILE-LAYOUT §3).

mod create;
mod error;
mod list;
mod model;

pub use create::workspace_create;
pub use error::WorkspacesError;
pub use list::workspace_list;
pub use model::WorkspaceRecord;
