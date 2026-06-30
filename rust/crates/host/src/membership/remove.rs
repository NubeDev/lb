//! `membership.remove` — leave / kick: drop the membership AND compose the shipped revoke seams for a
//! clean exit (global-identity scope, decision: "remove composes revoke_subject + revoke_tokens").
//! Gated by `mcp:members.manage:call`, workspace-first. Idempotent (removing an absent membership is a
//! success).
//!
//! It COMPOSES rather than duplicates: (1) tombstone the `membership:{sub}` row (raw
//! [`membership_remove_raw`](lb_authz::membership_remove_raw)); (2) [`token_revoke_mark`] — the
//! live-token marker the verify chokepoint reads, so the subject's CURRENT token is refused on the
//! next request (the freshness-asymmetry closer); (3) [`revoke_subject`] — tombstone every grant the
//! subject holds so caps drop on next re-mint. The subject's global identity is untouched; it may
//! still be a member of other workspaces.

use lb_auth::Principal;
use lb_authz::{membership_remove_raw, revoke_subject, token_revoke_mark, Subject};
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::MembershipError;

/// Remove `sub` from workspace `ws` as `principal`. Returns the number of grants tombstoned by
/// `revoke_subject` (the consequence/audit note). Idempotent.
pub async fn membership_remove(
    store: &Store,
    principal: &Principal,
    ws: &str,
    sub: &str,
) -> Result<usize, MembershipError> {
    authorize_tool(principal, ws, "members.manage").map_err(|_| MembershipError::Denied)?;
    membership_remove_raw(store, ws, sub).await?;
    let Some(name) = sub.strip_prefix("user:") else {
        return Ok(0);
    };
    let subject = Subject::User(name.to_string());
    token_revoke_mark(store, ws, &subject).await?;
    let revoked = revoke_subject(store, ws, &subject).await?;
    Ok(revoked)
}
