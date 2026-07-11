//! `lb-authz` — the durable authorization model: per-workspace **grants**, **roles** (cap
//! bundles), and **teams**, plus the rule by which a login session derives a token's caps from
//! them (authz-grants scope). The three-gate enforcement (`lb_caps::check` + `lb_assets::visibility`)
//! is **unchanged**; this crate fills Gate 2's input — caps become *administered data*, not a
//! hand-minted token.
//!
//! Shape, mirroring `lb-assets` relations: **raw, workspace-namespaced verbs, no authorization
//! here** — the host `authz` service is the capability chokepoint (capability-first §3.5). Every
//! record is `(table, id)` in the workspace's own namespace (the hard wall §7), so a ws-B caller
//! can never read or write ws-A's grants/roles/teams.
//!
//! Three record families, one verb per file (FILE-LAYOUT §3):
//!
//! | family | record / edge                        | verbs                                   |
//! |--------|--------------------------------------|-----------------------------------------|
//! | grant  | `grant(subject -> cap)`              | `assign` / `revoke` / `list`            |
//! | role   | `role(name -> caps[])`               | `define` / `list` ; assign = a grant    |
//! | team   | `team(team, name)` + `member` edges  | `create` / `list` ; members = `lb_assets` |
//!
//! [`resolve_caps`] is the session projection: `union(direct user grants, the user's roles' caps,
//! team-inherited grants)` for the workspace. The token it mints is a *cached projection* — the
//! **freshness asymmetry** (Gate 2 stale-until-remint, Gate 3 live) lives in that fact and is
//! documented on [`resolve_caps`].
//!
//! The **revoke seam** ([`revoke_subject`]) is the entry the `admin-crud` destructive verbs call to
//! strip a deleted user's/team's grants in one place — so deletion-revocation is not reimplemented
//! per caller.

mod check_scoped;
mod grant;
mod identity;
mod membership;
mod resolve;
mod resolve_scoped;
mod resolve_sourced;
mod revoke;
mod role;
mod scope;
mod subject;
mod team;
mod token_revoke;

pub use check_scoped::{check_scoped, check_scoped_with, scope_filter, scope_filter_with};
pub use grant::{
    grant_assign, grant_assign_scoped, grant_list, grant_list_scoped, grant_revoke,
    grant_revoke_scoped, granted, Grant, GRANT_TABLE,
};
pub use identity::{
    identity_create, identity_get, identity_list, Identity, IDENTITY_KIND, IDENTITY_NS,
    IDENTITY_TABLE,
};
pub use membership::{
    membership_add_raw, membership_get, membership_has_any, membership_is_member, membership_list,
    membership_remove_raw, Membership, MEMBERSHIP_KIND, MEMBERSHIP_TABLE, MEMBERSHIP_TOMBSTONE,
};
pub use resolve::{
    resolve_caps, resolve_caps_with, resolve_subject_caps, resolve_subject_caps_with,
    BuiltinRoleCaps, NoBuiltinRoleCaps,
};
pub use resolve_scoped::{resolve_caps_scoped, resolve_caps_scoped_with, ScopedCap};
pub use resolve_sourced::{
    resolve_caps_sourced, resolve_caps_sourced_with, resolve_subject_caps_sourced,
    resolve_subject_caps_sourced_with, CapSource, SourcedCap,
};
pub use revoke::revoke_subject;
pub use role::{role_caps, role_define, role_delete, role_list, Role, ROLE_TABLE};
pub use scope::{Scope, ScopeFilter};
pub use subject::Subject;
pub use team::{team_create, team_delete, team_list, Team, TEAM_TABLE};
pub use token_revoke::{token_revoke_mark, token_revoked, TOKEN_REVOKE_TABLE};

/// The `member` relation kind — the edge `team -[member]-> user` the S4 `visibility` resolver
/// reads and `lb_host::members` manages. Named here so the resolver can walk team membership.
pub const MEMBER: &str = "member";
