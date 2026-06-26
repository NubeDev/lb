//! [`GithubTarget`] — the real GitHub-HTTP impl of the host's `Target` trait (the outbox's delivery
//! seam). It turns an outbox effect into a GitHub REST `POST` and reports whether GitHub acknowledged
//! it, so `relay_outbox` can mark it delivered or (transiently) failed. This is the client peer of a
//! GitHub origin; like `HttpSource`, it lives in a role crate because it pulls in `reqwest`, which has
//! no place compiled into every node (roles depend on host, never the reverse).
//!
//! Three correctness points the outbox contract demands:
//!   1. **Idempotency.** The relay delivers at-least-once, so `deliver` MUST be a no-op on a
//!      re-delivery. For `create_pr`, GitHub itself is the dedup oracle: a second create for the same
//!      `head` returns `422` (a PR already exists) — we treat that as **success**, not a failure, so
//!      the relay marks it delivered and never opens two PRs. The effect's `idempotency_key` also
//!      rides as a header for a dedup-aware receiver/proxy.
//!   2. **Transient vs permanent.** A `5xx` / network error is transient → `Err` → the relay retries
//!      (with backoff). A mapping error (unknown action, bad payload) or an auth `401/403` is
//!      permanent for this effect → `Err` too, but the dead-letter cap stops the futile retries.
//!   3. **The token is mediated, never logged.** It is a private field, sent only as the
//!      `Authorization` header, and never appears in an error string.

use lb_host::Target;
use lb_outbox::Effect;

use crate::request::{to_request, MapError};

/// A GitHub delivery target pointed at an API base URL (`https://api.github.com`, or a test origin)
/// with a bearer token. One `reqwest::Client` is reused so connections pool across deliveries.
pub struct GithubTarget {
    base_url: String,
    token: String,
    client: reqwest::Client,
}

impl GithubTarget {
    /// Build a target against `base_url` (no trailing slash needed) authenticating with `token`. The
    /// token is held privately and only ever sent as the `Authorization` header.
    pub fn new(base_url: impl Into<String>, token: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            token: token.into(),
            client: reqwest::Client::new(),
        }
    }
}

impl Target for GithubTarget {
    async fn deliver(&self, effect: &Effect) -> Result<(), String> {
        // Pure mapping first — a permanent fault (wrong target, unknown action, bad payload) fails
        // without an HTTP round-trip. The relay's dead-letter cap parks an effect that maps wrong.
        let req = to_request(effect).map_err(map_err)?;
        let url = format!("{}{}", self.base_url, req.path);

        let resp = self
            .client
            .post(&url)
            .header("authorization", format!("Bearer {}", self.token))
            .header("accept", "application/vnd.github+json")
            // The dedup key for a receiver/proxy that honours it (real GitHub ignores unknown
            // headers; create_pr's natural 422-dedup below is the real guarantee).
            .header("idempotency-key", &effect.idempotency_key)
            .json(&req.body)
            .send()
            .await
            // A transport failure (DNS / refused / timeout) is transient — retry with backoff.
            .map_err(|e| format!("github {}: transport: {}", effect.idempotency_key, e))?;

        let status = resp.status();
        if status.is_success() {
            return Ok(());
        }
        // create_pr is idempotent via GitHub: a 422 "pull request already exists" means a prior
        // at-least-once attempt already opened it — treat as delivered, not a failure (no double-PR).
        if status == reqwest::StatusCode::UNPROCESSABLE_ENTITY && effect.action == "create_pr" {
            return Ok(());
        }
        // Anything else is a failure. 5xx is transient (retry); 4xx is effectively permanent but
        // still surfaced as an error — the dead-letter cap stops the retries. The body is NOT echoed
        // wholesale (it can carry sensitive context); only the status rides the error.
        Err(format!(
            "github {}: HTTP {}",
            effect.idempotency_key,
            status.as_u16()
        ))
    }
}

/// Render a permanent mapping error as a delivery error string (the token/payload never appear).
fn map_err(e: MapError) -> String {
    match e {
        MapError::WrongTarget => "github: effect is not a github target".into(),
        MapError::UnknownAction(a) => format!("github: unknown action {a}"),
        MapError::BadPayload(m) => format!("github: bad payload: {m}"),
    }
}
