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

mod grant;
mod resolve;
mod revoke;
mod role;
mod subject;
mod team;

pub use grant::{grant_assign, grant_list, grant_revoke, granted, Grant, GRANT_TABLE};
pub use resolve::resolve_caps;
pub use revoke::revoke_subject;
pub use role::{role_caps, role_define, role_list, Role, ROLE_TABLE};
pub use subject::Subject;
pub use team::{team_create, team_delete, team_list, Team, TEAM_TABLE};

/// The `member` relation kind — the edge `team -[member]-> user` the S4 `visibility` resolver
/// reads and `lb_host::members` manages. Named here so the resolver can walk team membership.
pub const MEMBER: &str = "member";
