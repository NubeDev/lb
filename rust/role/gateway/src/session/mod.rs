//! The gateway's **session** — the real identity seam this slice adds (collaboration scope, slice
//! 1). Two verbs, one per file (FILE-LAYOUT §3):
//!   - [`authenticate`] — read the `Authorization: Bearer <token>` header, `lb_auth::verify` it
//!     with the node key, and return the verified [`Principal`]. EVERY guarded route calls this
//!     first; the workspace + caps come from the token, never the request (the hard wall, §7).
//!   - [`credentials`] — the dev-login: map a `(user, workspace)` to a claim set. This is the
//!     ONLY non-real piece (Non-goals: no IdP yet); the token it mints is fully real (signed,
//!     verified). A real credential check / IdP plugs in here behind the same `verify` seam.

mod authenticate;
mod credential;
mod credentials;
pub mod events;
mod reach;
mod trusted;

pub use authenticate::{authenticate, verify_token, AuthRejection};
pub use credential::{
    credential_check_from_env, CredentialCheck, CredentialRejection, DevTrustAny, PasswordHash,
    DEV_LOGIN_ENV,
};
pub use credentials::dev_claims;
pub use reach::require_reach;
pub use trusted::trusted_from_env;
