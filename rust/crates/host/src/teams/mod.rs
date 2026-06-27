//! The **teams** service — the destructive half of the team lifecycle (admin-crud scope), completing
//! `authz`'s `teams.create`/`list`. `teams.delete` cascades (drop member edges + revoke team grants +
//! tombstone the record, one logical op); `teams.rename` updates the display name. Both gated by the
//! dedicated `mcp:teams.manage:call` admin cap (the same cap `members.add`/`remove` reuse), through
//! `authorize_tool`, workspace-first. The MCP bridge ([`call_teams_tool`]) exposes them.

mod delete;
mod error;
mod rename;
mod tool;

pub use delete::teams_delete;
pub use error::TeamsError;
pub use rename::teams_rename;
pub use tool::call_teams_tool;
