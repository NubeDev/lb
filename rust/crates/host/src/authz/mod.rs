//! The **authz** service — the host's capability chokepoint for the durable authorization model:
//! grants, roles, and teams (authz-grants scope). Wraps the raw `lb_authz` store verbs with the MCP
//! gate (capability-first §3.5, isolation-first §3.6) through `authorize_tool`, and adds the
//! no-widening rule (an admin can only grant/bundle caps they themselves hold — `hold.rs`).
//!
//! Verbs (one concern per file, FILE-LAYOUT §3): `grants.assign`/`revoke`/`list`,
//! `roles.define`/`list`, `teams.create`/`list`. The MCP bridge ([`call_authz_tool`]) exposes them
//! under the one MCP contract.
//!
//! Two **seams** are re-exported for the `admin-crud` slice (which consumes this model, not
//! duplicates it): [`resolve_caps`] (the session's login-time cap projection) and [`revoke_subject`]
//! (revocation-on-delete). Keeping them here means the destructive verbs call one model, not two.

mod builtin_caps;
mod builtin_roles;
mod error;
mod grant_ui;
mod grants;
mod hold;
mod resolve;
mod resolve_live;
mod revoke_tokens;
mod roles;
mod scoped;
mod teams;
mod tool;

pub use builtin_caps::LiveBuiltinRoleCaps;
pub use builtin_roles::{
    admin_only_caps, author_caps, ensure_builtin_authz_roles, member_role_caps, viewer_role_caps,
    workspace_admin_role_caps, ROLE_MEMBER, ROLE_VIEWER, ROLE_WORKSPACE_ADMIN,
};
pub use error::AuthzError;
pub use grant_ui::grant_ui_scope_to_admin;
pub use grants::{grants_assign, grants_list, grants_list_scoped, grants_revoke};
pub use hold::holds_cap;
pub use resolve::authz_resolve;
pub use resolve_live::{resolve_caps_live, resolve_subject_caps_live};
pub use revoke_tokens::revoke_tokens;
pub use roles::{roles_define, roles_delete, roles_list};
pub use scoped::{authz_check_scoped, authz_scope_filter};
pub use teams::{teams_create, teams_list};
pub use tool::call_authz_tool;

// The model types + the two seams admin-crud consumes — re-exported so host callers and the gateway
// use one set (the same way `tags` re-exports its `lb_tags` value types).
pub use lb_authz::{
    resolve_caps, revoke_subject, token_revoked, CapSource, Grant, Role as AuthzRole, Scope,
    ScopeFilter, SourcedCap, Subject, Team,
};
