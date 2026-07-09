//! The **credential** service — the per-`(workspace, user)` password credential store behind the
//! login credential check (login-hardening scope). The one non-real piece of the auth stack (the
//! dev-login) becomes real *here*: an admin sets a user's password via the mediated
//! `identity.set_credential` verb (gated `mcp:identity.manage:call`), and the gateway's `PasswordHash`
//! check verifies a login secret against the stored argon2 hash before minting a token.
//!
//! Everything is workspace-namespaced (the hard wall §7): a password set in `acme` cannot
//! authenticate a login into `beta`. The plaintext flows only through `set` (hashed) and `verify`
//! (compared) and is NEVER returned by a read — there is no `list`/get of the hash (secrets §6.7).
//!
//! One concern per file (FILE-LAYOUT §3): `model` (the record) / `hash` (argon2) / `set` (the admin
//! write verb) / `verify` (the login-path check) / `tool` (the MCP bridge) / `error`.

mod error;
mod hash;
mod model;
mod set;
mod tool;
mod verify;

pub use error::CredentialError;
pub use set::identity_set_credential;
pub use tool::call_credential_tool;
pub use verify::{credential_verify, CredentialCheck};
