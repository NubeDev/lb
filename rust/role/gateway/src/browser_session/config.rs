//! `BrowserSessionConfig` — the opt-in switch for the `/api/*` browser-session seam
//! (browser-session scope).
//!
//! **Role is config, never a code branch (rule 2).** `None` on the gateway ⇒ the router is exactly
//! today's: no `/api/*` routes exist, no cookie is ever set, and rubixd / rubix-ai / every bearer-only
//! node is untouched byte-for-byte. `Some(cfg)` ⇒ the gateway mounts the seam. There is no `if host ==
//! ems` anywhere: the gateway learns only "a shell is being served and sessions are cookies".

/// How the gateway terminates a browser session for a host that serves a shell.
///
/// `#[non_exhaustive]` + `Default` with all-`pub` fields, matching `BootConfig`'s posture: a
/// downstream embedder constructs this by mutating `default()`, so a new knob (an idle timeout, a
/// cookie name) lands as an additive field without breaking anyone. The `..Default::default()` form
/// is the supported construction path.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct BrowserSessionConfig {
    /// Session lifetime in seconds. The cookie's sid resolves to a stored token until this elapses,
    /// then reads as absent (a `401`, never a 500) and the row is GC'd.
    pub ttl_secs: u64,
    /// Mark the cookie `Secure` (TLS-only). **Off by default on purpose**: the deploys this seam
    /// exists for (an ARM/Pi box on a LAN, `http://pi.local:8391`) are plain-http, and a `Secure`
    /// cookie there is silently dropped by the browser — which is precisely the "login does nothing"
    /// class of bug this whole scope is fixing. An embedder terminating TLS sets this `true`.
    pub secure_cookie: bool,
}

/// The default session lifetime: 12h, matching the token's own `SESSION_TTL_SECS` so the cookie and
/// the JWT it stands for expire together (a sid outliving its token would 401 confusingly from
/// downstream routes instead of cleanly at the seam).
pub const DEFAULT_SESSION_TTL_SECS: u64 = 60 * 60 * 12;

impl Default for BrowserSessionConfig {
    fn default() -> Self {
        Self {
            ttl_secs: DEFAULT_SESSION_TTL_SECS,
            secure_cookie: false,
        }
    }
}
