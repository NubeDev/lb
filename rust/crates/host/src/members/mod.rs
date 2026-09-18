//! The **members** service — the read/add surface over the S4 membership backend, exposed for the
//! collaboration UI (collaboration scope, slice 3). The team `member` edges already exist
//! (`assets::add_member` writes them, `visibility::may_read_doc` resolves them); this service makes
//! them *enumerable* (`list_members`) and *addable through a real verb* (`add_member`) so the UI can
//! show "who is on this team" and add someone.
//!
//! Authorization is the MCP gate (`mcp:members.<verb>:call`) through the shared `authorize_tool`
//! chokepoint (workspace-first §7, then capability §3.5) — the same gate every MCP surface uses. The
//! raw edge persistence stays in `lb_assets`; this layer is authorization + the membership graph read.
//! The MCP bridge ([`call_members_tool`]) exposes them. It was missing for the whole life of this
//! service — the caps and the catalog rows existed, so the verbs looked shipped while every call
//! answered `no such tool`; the assign picker was the surface that showed it.
//!
//! One verb per file (FILE-LAYOUT §3). Minimal by design (the open question's lean: list members,
//! add a member; full team CRUD is a follow-up).

mod add;
mod error;
mod list;
mod remove;
mod tool;

pub use add::add_member as add_team_member;
pub use error::MembersError;
pub use list::list_members;
pub use remove::remove_member;
pub use tool::call_members_tool;
