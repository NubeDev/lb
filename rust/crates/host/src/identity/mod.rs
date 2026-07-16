//! The **identity** service — the global identity directory verbs over the reserved `_lb_identity`
//! namespace (global-identity scope). The genuinely-new primitive: before this, "a user" was a row
//! inside a workspace namespace, so `bob` in `acme` and `bob` in `globex` were unrelated records and
//! the workspace switcher could not carry one identity across. Now a person is one record in a system
//! directory, linked to workspaces by a `membership` row (the `membership` service).
//!
//! Verbs (one per file, FILE-LAYOUT §3): `identity.create` / `identity.get` / `identity.list` /
//! `identity.workspaces`, each gated `mcp:identity.manage:call` through `authorize_tool`. Identity +
//! membership WRITES are hub-only (decision #8) — the gateway (hub) serves them; an edge-role node
//! does not mount them. The MCP bridge ([`call_identity_tool`]) exposes them under the one contract.

mod by_email;
mod create;
mod error;
mod get;
mod list;
mod login_workspaces;
mod model;
mod set_email;
mod tool;
mod workspaces;

pub use by_email::identity_by_email;
pub use create::identity_create;
pub use error::IdentityError;
pub use get::identity_get;
pub use list::identity_list;
pub use login_workspaces::login_workspaces;
pub use model::{IdentityView, IdentityWorkspace};
pub use set_email::identity_set_email;
pub use tool::call_identity_tool;
pub use workspaces::identity_workspaces;

// Crate-internal: the effective-member helpers the membership service + login share.
pub(crate) use workspaces::{has_any_effective_member, is_effective_member};
