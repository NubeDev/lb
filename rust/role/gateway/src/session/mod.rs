//! The gateway's **session** — the real identity seam this slice adds (collaboration scope, slice
//! 1). Two verbs, one per file (FILE-LAYOUT §3):
//!   - [`authenticate`] — read the `Authorization: Bearer <token>` header, `lb_auth::verify` it
//!     with the node key, and return the verified [`Principal`]. EVERY guarded route calls this
//!     first; the workspace + caps come from the token, never the request (the hard wall, §7).
//!   - [`credentials`] — the claim builder: map a `(sub, workspace)` to a claim set, which
//!     [`mint_session`] then unions with the caller's resolved role/grant/nav-reach caps. The
//!     credential itself is proven by [`global_credential`] at `/auth/login`, the only human door.

mod authenticate;
mod credentials;
pub mod events;
mod global_credential;
mod mint_session;
mod reach;
mod select_token;
mod trusted;

pub use authenticate::{authenticate, verify_token, AuthRejection};
pub use credentials::dev_claims;
pub use global_credential::{
    global_credential_check_from_env, CredentialRejection, GlobalCredentialCheck,
    GlobalDevTrustAny, GlobalPasswordHash, DEV_LOGIN_ENV,
};
pub use mint_session::{
    mint_full_session, mint_full_session_with_ttl, MintedSession, SESSION_TTL_SECS,
};
pub use reach::require_reach;
pub use select_token::{is_select_token, mint_select_token, SELECT_TTL_SECS, WS_SELECT_CONSTRAINT};
pub use trusted::{authenticity_from_env, trusted_from_env};
