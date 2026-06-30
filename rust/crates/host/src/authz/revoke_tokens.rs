//! `authz.revoke-tokens` — the **live-token revoke lever** (access-console scope). Gated
//! `mcp:authz.revoke-tokens:call`, admin-only. It COMPOSES the two halves of a full immediate
//! lockout rather than reimplementing either:
//!   - the **live-token** half — write a `token_revoke` tombstone the verify chokepoint checks, so
//!     the subject's *current* (cached) token is refused on the next request (the genuinely-new
//!     piece this scope adds); and
//!   - the **grant-revoke** half — call the shipped [`revoke_subject`](lb_authz::revoke_subject),
//!     which tombstones every grant the subject holds so the caps drop on the next re-mint.
//!
//! One admin "Apply now — end active sessions" action calls this. The single-node case is instant
//! (the next verify reads the marker); the multi-node worst-case window is bounded by the token TTL
//! (the marker syncs idempotently under §6.8) — stated honestly in the UI copy, never "instant
//! global revoke". Returns the number of grants tombstoned (the consequence note).

use lb_auth::Principal;
use lb_authz::{revoke_subject, token_revoke_mark, Subject};
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::AuthzError;

/// Revoke `subject`'s live tokens (the verify-path marker) AND tombstone every grant it holds, in
/// `ws`. Idempotent. Returns the number of grants tombstoned by `revoke_subject`.
pub async fn revoke_tokens(
    store: &Store,
    principal: &Principal,
    ws: &str,
    subject: &Subject,
) -> Result<usize, AuthzError> {
    authorize_tool(principal, ws, "authz.revoke-tokens").map_err(|_| AuthzError::Denied)?;
    // Live-token half: write the marker the verify path reads on the next request. First, so even a
    // subject with zero grants still has its current token refused.
    token_revoke_mark(store, ws, subject).await?;
    // Grant-revoke half: tombstone every grant (next-re-mint). Composes — does not duplicate.
    let revoked = revoke_subject(store, ws, subject).await?;
    Ok(revoked)
}
