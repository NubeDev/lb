//! The **invites** service — durable, revocable, single-use token onboarding for people who don't
//! exist yet (invites scope). An admin mints an invite (carrying role/team intent + opaque
//! payload), the outbox delivers the email, and a pre-auth accept route redeems the token into
//! `identity` + `membership` + grants — atomically, with caps live on first login.
//!
//! Verbs (one concern per file): `create` / `list` / `revoke` / `resend` / `accept`. The MCP
//! bridge (`tool.rs`) exposes the admin verbs; `accept` is the pre-auth route the gateway serves
//! at `POST /public/invite/accept`.

mod accept;
mod create;
mod error;
mod list;
mod revoke;
mod token;
mod tool;

pub use accept::{invite_accept, AcceptedInvite};
pub use create::{invite_create, EMAIL_ACTION, EMAIL_TARGET};
pub use error::InviteError;
pub use list::invite_list;
pub use revoke::{invite_resend, invite_revoke};
pub use tool::call_invite_tool;
