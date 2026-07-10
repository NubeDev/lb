//! Host entry points over the resolver that bake in [`LiveBuiltinRoleCaps`] — the live built-in
//! cap bundles (builtin-role-freshness scope). These are what host callers SHOULD use: a new
//! built-in cap reaches already-seeded workspaces without a re-seed. The raw `lb_authz::resolve_caps`
//! / `resolve_subject_caps` (no builtins) stay available for tests that want the stored-row fold.
//!
//! Every host caller that resolves caps for a token/principal — the login mint (`role/gateway`),
//! the apikey auth/get path, the reminder fire re-resolve, the dashboard access_check — goes through
//! here so the fix is universal (one chokepoint, not five scattered `&LiveBuiltinRoleCaps` args).

use std::collections::BTreeSet;

use lb_authz::{resolve_caps_with, resolve_subject_caps_with, Subject};
use lb_store::{Store, StoreError};

use crate::authz::LiveBuiltinRoleCaps;

/// Resolve `user`'s effective caps in workspace `ws`, UNIONING the live built-in role bundles on top
/// of the stored records — the host's canonical resolve entry point. See [`resolve_caps_with`].
pub async fn resolve_caps_live(
    store: &Store,
    ws: &str,
    user: &str,
) -> Result<Vec<String>, StoreError> {
    resolve_caps_with(store, ws, user, &LiveBuiltinRoleCaps).await
}

/// [`resolve_subject_caps_with`] with [`LiveBuiltinRoleCaps`] baked in — the host's canonical entry
/// for a `key:`/`team:`/`role:` subject (no team-membership edge).
pub async fn resolve_subject_caps_live(
    store: &Store,
    ws: &str,
    subject: &Subject,
    caps: &mut BTreeSet<String>,
) -> Result<(), StoreError> {
    resolve_subject_caps_with(store, ws, subject, &LiveBuiltinRoleCaps, caps).await
}
