//! `panel.delete(id, {force?})` — tombstone-upsert (library-panels scope, "MCP surface"; §6.8
//! idempotent). Owner-only, like update. Gated by `mcp:panel.delete:call`.
//!
//! **Delete-safety** (library-panels scope, the risk callout): a delete of a panel still referenced by
//! dashboards is **refused** with the usage list ([`PanelError::InUse`]) unless `force` is set. A
//! forced tombstone leaves referencing cells to hydrate to the "panel deleted" placeholder until they
//! are relinked/removed. A re-delete or a delete of an absent panel is an idempotent no-op.

use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_panel;
use super::error::PanelError;
use super::store::{read_panel, write_panel};
use super::usage::scan_usage;

/// Soft-delete panel `id` in `ws` as `principal`, at logical time `now`. Idempotent: an absent or
/// already-tombstoned panel is a no-op. Only the owner may delete. Refuses (returns [`PanelError::InUse`])
/// when dashboards still reference it and `force` is false.
pub async fn panel_delete(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    force: bool,
    now: u64,
) -> Result<(), PanelError> {
    authorize_panel(principal, ws, "panel.delete")?;

    match read_panel(store, ws, id).await? {
        // Already gone (absent or tombstoned) — idempotent no-op.
        None => Ok(()),
        Some(p) if p.deleted => Ok(()),
        Some(mut p) => {
            if p.owner != principal.owner_sub() {
                return Err(PanelError::Denied);
            }
            // Delete-safety: refuse while referenced unless forced. Uses the cap-free `scan_usage`
            // (this call is already gated on `panel.delete`), so delete never demands `panel.usage`.
            if !force {
                let usage = scan_usage(store, principal, ws, id).await?;
                if !usage.is_empty() {
                    return Err(PanelError::InUse(usage));
                }
            }
            p.deleted = true;
            p.updated_ts = now;
            write_panel(store, ws, &p).await?;
            Ok(())
        }
    }
}
