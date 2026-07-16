//! The **global identity credential** service — one argon2id password per *person*, in the reserved
//! `_lb_identity` namespace (email-login scope, the decision-#7 `cred_ref` seam made real). This is
//! the human login credential for the `/auth/*` front door: a person has ONE password across all
//! their workspaces (Slack-style), distinct from the shipped per-`(workspace, user)` `Credential`
//! that backs the legacy `POST /login`.
//!
//! Verbs, one concern per file (FILE-LAYOUT §3):
//!   - `set` — the admin `identity.set_password` verb (gated `mcp:identity.manage:call`).
//!   - `change` — the self-service `POST /auth/password` backend (verify old, set new; no admin cap).
//!   - `verify` — the login-path `global_credential_verify` seam (timing-uniform on unknown identity).
//!   - `tool` — the MCP bridge for `identity.set_password`.
//!   - `error` — the service error (secret-free).
//! The record itself lives in `lb_authz` (`identity_credential`), the raw store layer.

mod change;
mod error;
mod set;
mod tool;
mod verify;

pub use change::identity_change_password;
pub use error::IdentityCredentialError;
pub use set::identity_set_password;
pub use tool::call_identity_credential_tool;
pub use verify::{global_credential_verify, GlobalCredentialCheck};
