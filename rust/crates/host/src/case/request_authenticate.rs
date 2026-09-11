//! **Presenting a token** — the one function that turns the string in a contractor's URL into a
//! principal (case-plane scope §"Gateway — the token principal").
//!
//! It lives in the host, not the gateway, for the reason every other authentication chain does: the
//! gateway is transport, and "what a token means" is a property of the plane that minted it. The
//! route's job is the HTTP status and the rate limiter; this function's job is the wall.
//!
//! **The three outcomes, and why there are only three.**
//!   - [`RequestTokenError::NotFound`] — nothing hashes to this. The token is not, and never was, a
//!     link.
//!   - [`RequestTokenError::Gone`] — a real ask that can no longer be acted on: withdrawn, or past
//!     its window. **The caller is never told which**, and the gateway's body says so as a
//!     disjunction. "This request expired" tells a prober the string they guessed was real.
//!   - `Ok` — a live ask (including one already replied to inside its window, so the page can render
//!     the answer it already gave).
//!
//! Expiry is **derived and then persisted** ([`lb_cases::request_expire_if_due`]): a window that
//! closed while the node was down reads as expired the moment anyone looks, with no sweep to miss.

use lb_auth::Principal;
use lb_cases::CaseRequest;
use lb_store::Store;

use super::request_scope::token_principal;
use super::request_token::hash_request_token;

/// Why a presented token did not produce a principal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestTokenError {
    /// No request hashes to this token.
    NotFound,
    /// A real request that is withdrawn or past its window. Never says which.
    Gone,
    /// The store failed. Not a statement about the token.
    Store(String),
}

impl std::fmt::Display for RequestTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RequestTokenError::NotFound => write!(f, "no such request"),
            RequestTokenError::Gone => write!(f, "this request is no longer open"),
            RequestTokenError::Store(e) => write!(f, "store error: {e}"),
        }
    }
}

/// Resolve `token` in workspace `ws` at logical `now` (epoch-ms) into its principal and its row.
///
/// The token is hashed and matched against the stored hash — the raw token is never compared, and
/// never stored. Every open is recorded on the row (`opened_ts` on the first one), which is what
/// makes "they read it and said nothing" a fact rather than a guess.
pub async fn case_request_authenticate(
    store: &Store,
    ws: &str,
    token: &str,
    now: u64,
) -> Result<(Principal, CaseRequest), RequestTokenError> {
    let hash = hash_request_token(token);
    let found = lb_cases::request_by_token_hash(store, ws, &hash)
        .await
        .map_err(|e| RequestTokenError::Store(e.to_string()))?;
    let Some(mut request) = found else {
        return Err(RequestTokenError::NotFound);
    };

    // Persist a window that has closed, so the row stops claiming it is waiting on anyone.
    lb_cases::request_expire_if_due(store, ws, &mut request, now)
        .await
        .map_err(|e| RequestTokenError::Store(e.to_string()))?;

    if !can_still_act(&request, now) {
        return Err(RequestTokenError::Gone);
    }

    let principal = token_principal(ws, &request.party_id, &request.id);
    Ok((principal, request))
}

/// Whether a presented token may still do anything at all. A **replied** request inside its window
/// is still viewable — the page must be able to show the party what they already told us.
fn can_still_act(request: &CaseRequest, now: u64) -> bool {
    use lb_cases::RequestStatus;
    match request.status {
        RequestStatus::Withdrawn | RequestStatus::Expired => false,
        _ => now < request.expires_ts,
    }
}
