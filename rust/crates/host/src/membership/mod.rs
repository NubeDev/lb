//! The **membership** service — the per-workspace roster verbs (global-identity scope). This is the
//! invite/join + leave surface: `membership.add` / `membership.remove` / `membership.list`, each gated
//! `mcp:members.manage:call` through `authorize_tool`. Membership rows are the **only** source of the
//! roster: the legacy `user:*` admin rows and the `membership_login_resolve` bootstrap seam were
//! removed with `POST /login` (email-login scope, removal sweep) — `/auth/*` is the only human door
//! and an operator provisions the first admin explicitly.
//!
//! `membership.add` writes the `membership:{sub}` row AND grants the built-in `member` role (a system
//! effect via the raw `grant_assign`, NOT the gated `grants_assign` — a system join is not a caller
//! widening). `membership.remove` tombstones the row AND composes the shipped `revoke_subject` +
//! `token_revoke_mark` (it does not duplicate them) for a clean exit. `membership.list` returns the
//! roster from the `membership` rows — one source of truth, the same rows `identity.workspaces`
//! reads, so the People tab and the login path can never disagree.
//!
//! One verb per file (FILE-LAYOUT §3). The MCP bridge ([`call_membership_tool`]) exposes them.

mod add;
mod error;
mod list;
mod model;
mod remove;
mod tool;

pub use add::membership_add;
pub use error::MembershipError;
pub use list::membership_list;
pub use model::MembershipView;
pub use remove::membership_remove;
pub use tool::call_membership_tool;
