//! The **browser session seam** (`/api/*`) — browser-session scope.
//!
//! lb's gateway is bearer-only, which is the right contract for a CLI, a sibling node, or rubixd, and
//! the wrong one for a browser: the token is the whole authority, and anywhere JS can read it, one XSS
//! is a total account compromise. lb also mints a *fat* token (the full resolved cap set, ~4–9KB) —
//! over the browser cookie limit, so "just cookie the JWT" does not merely risk something, it silently
//! fails. Two hosts (ems, cc-app) hit this independently and both solved it the same way: a dev-only
//! Vite middleware keeping the token server-side, cookieing a short opaque session id, and forwarding
//! `/api/*` with the bearer attached. Neither had a production equivalent, because **lb's gateway is
//! already their web server** (`static_root`) — so ems's ARM/Pi build served its shell and could not
//! log in. This module is that seam, owned once, here.
//!
//! **Opt-in, inert by default (rule 2: role = config, never a code branch).**
//! `Gateway::browser_session == None` ⇒ none of these routes are mounted, no cookie is ever set, and
//! the router is byte-for-byte today's. rubixd and rubix-ai are unaffected.
//!
//! One responsibility per file (FILE-LAYOUT):
//!   - [`config`] — the opt-in switch + TTL/`Secure` posture.
//!   - [`sid`] — CSPRNG session ids (explicitly not the dev plugins' guessable counter).
//!   - [`cookie`] — parse/emit, `HttpOnly; SameSite=Lax`.
//!   - [`csrf`] — the `Origin`/`Sec-Fetch-Site` gate on unsafe methods. **The gate on this shipping.**
//!   - [`store`] — store-backed sessions with a TTL, so a deploy doesn't log everyone out.
//!   - [`auth`] — `/api/auth/{login,select,switch,logout,session}`, reusing the real handlers.
//!   - [`forward`] — `ANY /api/{*rest}` → internal dispatch with the bearer attached.
//!
//! **Prior art to reconcile:** `docs/scope/deploy/rubixd/token-auth-scope.md` and `embedded-ui-scope.md`
//! say "no sessions/cookies — the bearer is the whole story". Those are **rubixd** scopes (a fleet
//! agent's own embedded UI), not the app-shell surface, and this seam is opt-in — so they remain true
//! where they were written. Both carry an annotation pointing here.

pub mod auth;
pub mod config;
pub mod cookie;
pub mod csrf;
pub mod forward;
pub mod sid;
pub mod store;

pub use config::{BrowserSessionConfig, DEFAULT_SESSION_TTL_SECS};
pub use forward::ApiState;
