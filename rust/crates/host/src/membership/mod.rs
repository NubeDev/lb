//! The **membership** service — the per-workspace roster verbs (global-identity scope). This is the
//! invite/join + leave surface: `membership.add` / `membership.remove` / `membership.list`, each gated
//! `mcp:members.manage:call` through `authorize_tool`, plus the un-gated `membership_login_resolve`
//! seam the gateway login route calls (no principal yet — it decides whether to mint).
//!
//! `membership.add` writes the `membership:{sub}` row AND grants the built-in `member` role (a system
//! effect via the raw `grant_assign`, NOT the gated `grants_assign` — a system join is not a caller
//! widening). `membership.remove` tombstones the row AND composes the shipped `revoke_subject` +
//! `token_revoke_mark` (it does not duplicate them) for a clean exit. `membership.list` returns the
//! **effective** roster = membership rows ∪ legacy `user:*` rows (lazy migration, decision #10).
//!
//! One verb per file (FILE-LAYOUT §3). The MCP bridge ([`call_membership_tool`]) exposes them.

mod add;
mod error;
mod list;
mod login_resolve;
mod model;
mod remove;
mod tool;

pub use add::membership_add;
pub use error::MembershipError;
pub use list::membership_list;
pub use login_resolve::{membership_login_resolve, WORKSPACE_ADMIN_ROLE_CAP};
pub use model::MembershipView;
pub use remove::membership_remove;
pub use tool::call_membership_tool;
