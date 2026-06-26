//! The **members** service — the read/add surface over the S4 membership backend, exposed for the
//! collaboration UI (collaboration scope, slice 3). The team `member` edges already exist
//! (`assets::add_member` writes them, `visibility::may_read_doc` resolves them); this service makes
//! them *enumerable* (`list_members`) and *addable through a real verb* (`add_member`) so the UI can
//! show "who is on this team" and add someone.
//!
//! Authorization is the MCP gate (`mcp:members.<verb>:call`) through the shared `authorize_tool`
//! chokepoint (workspace-first §7, then capability §3.5) — the same gate every MCP surface uses. The
//! raw edge persistence stays in `lb_assets`; this layer is authorization + the membership graph read.
//! One verb per file (FILE-LAYOUT §3). Minimal by design (the open question's lean: list members,
//! add a member; full team CRUD is a follow-up).

mod add;
mod error;
mod list;

pub use add::add_member as add_team_member;
pub use error::MembersError;
pub use list::list_members;
