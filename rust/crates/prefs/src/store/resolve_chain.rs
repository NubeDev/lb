//! `resolve_chain` — load the stored links (user pref, workspace default) for `(ws, user)` and fold
//! them into a [`ResolvedPrefs`] with `resolve::resolve` (prefs scope `prefs.resolve`). An optional
//! per-call `override_` is prepended as the highest-priority link (the self-scoped request override —
//! "preview in es" — that never writes the record).
//!
//! Only `ws`'s namespace is read (the hard wall): a resolve in ws-B can structurally never see
//! ws-A's record. Raw verb; the host gate (`prefs.resolve` = read OWN) runs first.

use lb_store::{Store, StoreError};

use crate::prefs::{Prefs, ResolvedPrefs};
use crate::resolve::resolve;

use super::default_get::get_workspace_prefs;
use super::get::get_user_prefs;

/// Resolve `(ws, user)`'s preferences. `override_` (if any) wins each axis it sets; then the user's
/// stored prefs, then the workspace default, then the built-in fallback.
pub async fn resolve_chain(
    store: &Store,
    ws: &str,
    user: &str,
    override_: Option<Prefs>,
) -> Result<ResolvedPrefs, StoreError> {
    let user_prefs = get_user_prefs(store, ws, user).await?;
    let ws_default = get_workspace_prefs(store, ws).await?;

    let mut links: Vec<Prefs> = Vec::with_capacity(3);
    if let Some(o) = override_ {
        links.push(o);
    }
    if let Some(u) = user_prefs {
        links.push(u);
    }
    if let Some(d) = ws_default {
        links.push(d);
    }
    Ok(resolve(&links))
}
