//! `ext.versions` — every catalog version on record for one extension in a workspace, newest first
//! (lifecycle-management scope: the version-history read gap). `CatalogEntry` already retains one row
//! per published `(ext_id, version)` — `record_catalog` upserts by that composite key, never
//! overwriting a prior version's row — but `ext.list`/`GET /extensions` only ever surfaces the
//! current, last-write-wins `Install` row. This is a pure read-side projection over the catalog that
//! already exists: no new persistence, and it touches nothing the `Install`/current-pointer semantics
//! depend on. Gated `mcp:ext.versions:call`, the read-verb peer of `ext.list`.

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_registry::CatalogEntry;

use super::error::ExtError;
use crate::boot::Node;
use crate::registry::list_catalog;

/// Every catalog entry recorded for `ext_id` in workspace `ws`, newest first. Empty (not an error) if
/// the extension has never been published here — same "no leak" posture `ext.list` uses for an
/// absent row.
pub async fn ext_versions(
    node: &Node,
    caller: &Principal,
    ws: &str,
    ext_id: &str,
) -> Result<Vec<CatalogEntry>, ExtError> {
    authorize_tool(caller, ws, "ext.versions").map_err(|_| ExtError::Denied)?;
    let mut versions = list_catalog(&node.store, ws, ext_id).await?;
    versions.sort_by(|a, b| b.ts.cmp(&a.ts));
    Ok(versions)
}
