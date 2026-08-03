// Raise the trait-solver recursion limit: proving `Sync` for the deeply-nested rhai AST type
// (`rhai::Dynamic` in a route handler's captured state, under `axum::routing::post`'s `Handler`
// bound) overflows the default 128 on a COLD compile (warm incremental builds dodge it). rhai's
// `sync` feature is unified on in this graph, so the type IS `Sync` — a depth limit, not a real
// `!Sync`. Latent pre-existing on master; a fresh cold build of the response-cache test suite
// surfaced it.
#![recursion_limit = "512"]
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

/// The browser-session seam (browser-session scope): the opt-in `/api/*` cookie session a host that
/// serves a shell (`static_root`) needs, so a browser never holds the bearer token. `pub` so an
/// embedder can name [`BrowserSessionConfig`] and a test can reach the cookie/CSRF primitives.
pub mod browser_session;
mod routes;
mod server;
/// The session seam (login-hardening scope): the credential check trait + impls (`DevTrustAny` /
/// `PasswordHash`) and the token authenticate/mint helpers. `pub` so a test can wire a gateway with
/// the real `PasswordHash` check (the production posture) instead of the password-less dev default.
pub mod session;
mod signing_key;
mod spa_fallback;
mod state;

pub use routes::{INVITE_ACCEPT_MAX_PER_WINDOW, INVITE_ACCEPT_WINDOW_SECS};
pub use server::{router, serve, serve_listener};
pub use session::{authenticate, dev_claims, verify_token, AuthRejection};
pub use session::{mint_full_session, mint_full_session_with_ttl, MintedSession, SESSION_TTL_SECS};
// The publisher trust-gate posture — re-exported at the crate root for the same reason as the
// credential seam: an embedder (via `lb-node`'s `BootConfig`) selects it without reaching into
// `session::trusted`, and the binary boundary is the one place `LB_EXT_UNTRUSTED_KEY` is read.
// `Authenticity` is re-exported from `lb-registry` so an embedder needs no direct dependency on it.
pub use lb_registry::Authenticity;
pub use session::authenticity_from_env;
// The credential-check seam (email-login, embedder-credential-mode scope) — re-exported at the crate
// root so an embedder (via `lb-node`'s builder) names the impls without reaching into the module. This
// is the ONLY credential seam; the per-ws `CredentialCheck` died with `POST /login`.
pub use session::{
    global_credential_check_from_env, GlobalCredentialCheck, GlobalDevTrustAny, GlobalPasswordHash,
};
// The browser-session opt-in (browser-session scope) — re-exported at the crate root so an embedder
// (via `lb-node`'s builder) names the config without reaching into the module.
pub use browser_session::{BrowserSessionConfig, DEFAULT_SESSION_TTL_SECS};
pub use state::Gateway;
