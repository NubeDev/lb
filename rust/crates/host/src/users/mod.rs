//! The **users** service — user records as administered data over the dev store (admin-crud scope).
//! The genuinely-missing primitive: before this, login auto-seeded a principal and you could not
//! list, disable, or delete a user. Now `(ws, user)` is a durable `user` record (`active`, `role`,
//! a mediated `cred_ref`), and the login path checks it.
//!
//! Verbs (one per file, FILE-LAYOUT §3): `user.create` / `user.list` / `user.disable` /
//! `user.enable` / `user.delete`, each gated `mcp:user.manage:call` (or `mcp:user.disable:call`)
//! through `authorize_tool`. Plus [`user_login_check`] — the un-gated pre-mint seam the gateway
//! login route calls so `disable`/`delete` bite minting. `user.delete` calls the slice-1 authz
//! revoke seam. The MCP bridge ([`call_users_tool`]) exposes the gated verbs under the one contract.

mod active;
mod create;
mod delete;
mod error;
mod list;
mod login_check;
mod model;
mod tool;

pub use active::{user_disable, user_enable};
pub use create::user_create;
pub use delete::user_delete;
pub use error::UsersError;
pub use list::user_list;
pub use login_check::user_login_check;
pub use model::UserView;
pub use tool::call_users_tool;
