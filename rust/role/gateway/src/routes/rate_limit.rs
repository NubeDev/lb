//! Per-IP rate limiting for the pre-auth invite accept route (invites scope: "the public route
//! ships rate-limited from day one"). `POST /public/invite/accept` is both an invite-token oracle
//! and — via `current_secret` — a password oracle, so it gets a hard request ceiling per client
//! before any handler logic runs.
//!
//! One responsibility (FILE-LAYOUT): a small in-memory **fixed-window** limiter + the axum
//! middleware that applies it. In-process state is correct here — the gateway is the node's one
//! HTTP front door, and a limiter is ephemeral motion-side state, not durable state (rule 3).
//!
//! Client key: the first `x-forwarded-for` hop when present (the deployed gateway sits behind a
//! proxy/ingress that sets it), else the literal `"direct"` bucket. The gateway serves without
//! `ConnectInfo` (tests drive routes via `oneshot`, no socket), so the header is the one portable
//! source of a client address; an unproxied deployment degrades to one shared bucket — strictly
//! *tighter*, never looser. Rejected alternative: a token-bucket per route via tower's
//! `RateLimitLayer` — it is global (not per-client) and buffers rather than rejects; a brute-force
//! defense must 429 the abuser without queueing everyone else.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

/// Max accept attempts per client key per window. Generous for a human retyping a password,
/// hopeless for a brute-force (10 guesses/minute).
pub const MAX_PER_WINDOW: u32 = 10;

/// The fixed window length, seconds.
pub const WINDOW_SECS: u64 = 60;

/// A fixed-window counter per client key. `allow` is the whole API: count the hit, say yes/no.
/// Windows are aligned to `now / window` so state is two words per key; stale keys are swept
/// opportunistically whenever the window rolls (the map never grows past one window's clients).
pub struct FixedWindowLimiter {
    max_per_window: u32,
    window_secs: u64,
    state: Mutex<LimiterState>,
}

struct LimiterState {
    /// The window index the counts belong to (`now / window_secs`).
    window: u64,
    /// Hits per client key within the current window.
    counts: HashMap<String, u32>,
}

impl FixedWindowLimiter {
    pub fn new(max_per_window: u32, window_secs: u64) -> Self {
        Self {
            max_per_window,
            window_secs,
            state: Mutex::new(LimiterState {
                window: 0,
                counts: HashMap::new(),
            }),
        }
    }

    /// Record a hit for `key` at `now` (epoch seconds) and return whether it is allowed.
    /// The (max+1)-th hit in a window — and every one after it — is refused.
    pub fn allow(&self, key: &str, now: u64) -> bool {
        let window = now / self.window_secs.max(1);
        let mut state = self.state.lock().expect("rate-limit state poisoned");
        if state.window != window {
            // The window rolled: every key starts fresh (and stale keys are dropped — the sweep).
            state.window = window;
            state.counts.clear();
        }
        let count = state.counts.entry(key.to_string()).or_insert(0);
        *count += 1;
        *count <= self.max_per_window
    }

    /// Whether `key` is still under the ceiling WITHOUT recording a hit — the read-only counterpart to
    /// [`allow`](Self::allow). Used where the count is driven separately (the `/auth/login` failure
    /// limiter records only on a failed attempt, but checks the ceiling on every attempt). Rolls the
    /// window like `allow` so a stale window reads as fresh (allowed).
    pub fn peek(&self, key: &str, now: u64) -> bool {
        let window = now / self.window_secs.max(1);
        let mut state = self.state.lock().expect("rate-limit state poisoned");
        if state.window != window {
            state.window = window;
            state.counts.clear();
            return true;
        }
        state.counts.get(key).copied().unwrap_or(0) < self.max_per_window
    }
}

/// The one process-wide limiter for the public invite route.
fn invite_limiter() -> &'static FixedWindowLimiter {
    static LIMITER: OnceLock<FixedWindowLimiter> = OnceLock::new();
    LIMITER.get_or_init(|| FixedWindowLimiter::new(MAX_PER_WINDOW, WINDOW_SECS))
}

/// The client key for a request: first `x-forwarded-for` hop, else the shared `"direct"` bucket
/// (see module docs — degrading to one bucket is tighter, never looser).
fn client_key(req: &Request) -> String {
    req.headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "direct".to_string())
}

/// Axum middleware for `POST /public/invite/accept`: 429 the client once it exceeds
/// [`MAX_PER_WINDOW`] hits in a [`WINDOW_SECS`] window. Applied to the public invite route ONLY
/// (session-authed routes are gated by caps, not this).
pub async fn invite_accept_rate_limit(req: Request, next: Next) -> Response {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if !invite_limiter().allow(&client_key(&req), now) {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            "rate limit exceeded — retry later",
        )
            .into_response();
    }
    next.run(req).await
}

// ---------------------------------------------------------------------------------------------
// The `/auth/login` per-email login limiter (email-login scope, resolved open question: 10 failures
// / 15 min per email, per-email only, v1). Unlike the invite limiter this is NOT middleware — it
// keys on the EMAIL in the request body and counts only FAILED attempts, so the login handler drives
// it directly: `auth_login_allowed(email, now)` before verifying, `auth_login_record_failure(email,
// now)` on a `401`. A successful login records nothing (a legit user is never locked out by their own
// success). In-memory per-node, exactly like the invite limiter — a distributed limiter is over-scope.

/// Max FAILED `/auth/login` attempts per email per window before the email is locked out.
pub const AUTH_LOGIN_MAX_FAILURES: u32 = 10;

/// The `/auth/login` failure window, seconds — 15 minutes (resolved open question).
pub const AUTH_LOGIN_WINDOW_SECS: u64 = 15 * 60;

/// The one process-wide per-email login-failure limiter.
fn auth_login_limiter() -> &'static FixedWindowLimiter {
    static LIMITER: OnceLock<FixedWindowLimiter> = OnceLock::new();
    // `max_per_window + 1` because `allow` counts the hit it is asked about: we want the (max+1)-th
    // FAILURE to be the one that trips, so the limiter's own ceiling sits one above the failure count.
    LIMITER.get_or_init(|| FixedWindowLimiter::new(AUTH_LOGIN_MAX_FAILURES, AUTH_LOGIN_WINDOW_SECS))
}

/// Is a `/auth/login` attempt for `email` still allowed (i.e. under the failure ceiling this window)?
/// Read-only — does NOT count the attempt. Call before verifying; a `false` return means the email is
/// locked out (respond `429`). `email` is folded (trimmed + lower-cased) by the caller so the bucket
/// matches the lookup.
pub fn auth_login_allowed(folded_email: &str, now: u64) -> bool {
    auth_login_limiter().peek(folded_email, now)
}

/// Record a FAILED `/auth/login` for `email` (wrong password / unknown email). Counts one hit toward
/// the window; the (max+1)-th failure trips [`auth_login_allowed`] to `false`. Call only on a `401`.
pub fn auth_login_record_failure(folded_email: &str, now: u64) {
    let _ = auth_login_limiter().allow(folded_email, now);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_up_to_max_then_refuses() {
        let l = FixedWindowLimiter::new(3, 60);
        assert!(l.allow("1.2.3.4", 100));
        assert!(l.allow("1.2.3.4", 101));
        assert!(l.allow("1.2.3.4", 102));
        assert!(!l.allow("1.2.3.4", 103), "4th hit in the window must 429");
        assert!(!l.allow("1.2.3.4", 104), "and it stays refused");
    }

    #[test]
    fn keys_are_independent() {
        let l = FixedWindowLimiter::new(1, 60);
        assert!(l.allow("a", 10));
        assert!(!l.allow("a", 11));
        assert!(l.allow("b", 12), "a different client is not punished");
    }

    #[test]
    fn window_roll_resets() {
        let l = FixedWindowLimiter::new(1, 60);
        assert!(l.allow("a", 59));
        assert!(!l.allow("a", 59));
        assert!(l.allow("a", 60), "the next window starts fresh");
    }

    #[test]
    fn peek_reports_the_ceiling_without_counting() {
        // The `/auth/login` failure limiter checks on every attempt but counts only failures.
        let l = FixedWindowLimiter::new(2, 60);
        // peek never records — a thousand peeks still leave the email allowed.
        assert!(l.peek("ada@x.com", 10));
        assert!(l.peek("ada@x.com", 10));
        assert!(l.peek("ada@x.com", 10), "peek does not increment");
        // Record two failures → the third attempt is now over the ceiling.
        assert!(l.allow("ada@x.com", 10));
        assert!(l.allow("ada@x.com", 10));
        assert!(
            !l.peek("ada@x.com", 10),
            "after max failures, peek reports locked out"
        );
        // A different email is unaffected.
        assert!(l.peek("bob@x.com", 10));
    }
}
