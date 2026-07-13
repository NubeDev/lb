//! The `docs.extract` capability chokepoint (doc-extraction scope) — gate 1 (workspace) + gate 2
//! (`mcp:docs.extract:call`), via the shared MCP authorizer. The job runs under the **caller's**
//! principal, never a widened service identity; per-item source-media read reach is a SEPARATE
//! gate re-checked per item inside the loop (`derive.rs`), so a caller who holds `docs.extract` but
//! cannot read one of three media ids gets that item `denied` while the other two extract.

use lb_auth::Principal;
use lb_mcp::authorize_tool;

use super::error::ExtractSvcError;

/// Gate the whole `docs.extract` request. `Ok(())` only if the caller may use the surface in `ws`.
pub fn authorize_extract(principal: &Principal, ws: &str) -> Result<(), ExtractSvcError> {
    authorize_tool(principal, ws, "docs.extract").map_err(|_| ExtractSvcError::Denied)
}

/// Gate per-item read reach on a source media id — the SAME gate `media.get` runs, under the
/// caller's principal (workspace-first). A denial here becomes the item's `denied` outcome, not a
/// job failure. Kept beside the request gate so both chokepoints read together.
pub fn may_read_media(principal: &Principal, ws: &str) -> bool {
    authorize_tool(principal, ws, "media.get").is_ok()
}
