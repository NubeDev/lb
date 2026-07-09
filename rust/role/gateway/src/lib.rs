//! Role-only: the **SSE/HTTP gateway** for browsers (README §6.13, frontend scope). A browser
//! reaches a REAL node here — POST to send, GET for durable history, and one SSE stream that
//! pushes *others'* live messages + presence. This replaces the S2 in-memory UI fake: the
//! `channel.api` verbs and `ChannelView` are unchanged; only `ui/src/lib/ipc/invoke.ts` swaps
//! its transport to point at this gateway.
//!
//! Symmetric nodes (§3.1): the gateway IS a node that also speaks HTTP — not a separate service.
//! It adds no authority; every route forwards to a capability-checked `lb_host` verb with the
//! session principal, so the browser is gated exactly like the desktop shell and the routed-MCP
//! caller. One verb per route file (FILE-LAYOUT §4).

mod routes;
mod server;
/// The session seam (login-hardening scope): the credential check trait + impls (`DevTrustAny` /
/// `PasswordHash`) and the token authenticate/mint helpers. `pub` so a test can wire a gateway with
/// the real `PasswordHash` check (the production posture) instead of the password-less dev default.
pub mod session;
mod signing_key;
mod state;

pub use server::{router, serve, serve_listener};
pub use session::{authenticate, dev_claims, verify_token, AuthRejection};
pub use state::Gateway;
