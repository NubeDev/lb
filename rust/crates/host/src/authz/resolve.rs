//! `authz.resolve` — the admin read that returns a subject's **resolved effective caps with
//! provenance** (access-console scope). Gated `mcp:authz.resolve:call`, admin-only, workspace-first.
//!
//! It is the gateway-side twin of the `resolve_caps`/`resolve_subject_caps` fold the session mint
//! runs — but sourced, so the access console shows *why* a subject holds each cap (direct / role /
//! via-team), not just *that* it does. Because it folds the SAME grants/roles/teams the mint folds,
//! the displayed set and the enforced set cannot drift (a cross-check test pins that). It is an
//! explicit, admin-only, on-demand read — never the hot path (the token is the cache).

use lb_auth::Principal;
use lb_authz::{resolve_caps_sourced_with, resolve_subject_caps_sourced_with, SourcedCap, Subject};
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::builtin_caps::LiveBuiltinRoleCaps;
use super::error::AuthzError;

/// Resolve `subject`'s effective caps in `ws`, each tagged with its source(s). A `user:` subject
/// folds direct ∪ roles ∪ team-inherited (the full session projection); any other subject kind
/// (`key:`/`team:`/`role:`) folds its own direct grants + roles (no team-membership edge — a key
/// joins no teams, a team IS the edge). Gated by `mcp:authz.resolve:call`.
///
/// Injects [`LiveBuiltinRoleCaps`] so the displayed set matches the minted token set — a new
/// built-in cap shows up here the moment code ships (builtin-role-freshness scope), keeping the
/// resolver↔mint cross-check exact.
pub async fn authz_resolve(
    store: &Store,
    principal: &Principal,
    ws: &str,
    subject: &Subject,
) -> Result<Vec<SourcedCap>, AuthzError> {
    authorize_tool(principal, ws, "authz.resolve").map_err(|_| AuthzError::Denied)?;
    match subject {
        Subject::User(user) => {
            Ok(resolve_caps_sourced_with(store, ws, user, &LiveBuiltinRoleCaps).await?)
        }
        _ => Ok(resolve_subject_caps_sourced_with(store, ws, subject, &LiveBuiltinRoleCaps).await?),
    }
}
