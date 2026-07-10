//! `report.save(id, title, blocks, brand_id, toolbar)` — one idempotent UPSERT for create+update
//! (reports scope; a fresh id creates, an existing id updates). Gated by `mcp:report.save:call`.
//!
//! **Ownership on update:** a save against an existing report is allowed only for its owner (a
//! non-owner with the save cap cannot overwrite someone else's report). Create stamps
//! `owner = principal`; `visibility` is set via `report.share`, so save **preserves** the existing
//! visibility.
//!
//! **Panel-block refs are validated + stripped:** each `panel` block's cell goes through the shipped
//! `lb_host::validate_and_strip_refs` (a `panel_ref` that does not resolve in-workspace is rejected
//! loudly → `BadInput`; the echoed spec is stripped so the ref stays authoritative). Inline panel
//! cells pass through untouched.

use lb_auth::Principal;
use lb_store::Store;
use serde_json::Value;

use super::authorize::authorize_report;
use super::error::ReportError;
use super::model::{Block, Report, Visibility, MAX_BLOCKS, SCHEMA_VERSION};
use super::store::{read_report, write_report};

/// Upsert report `id` in `ws` with `title` + `blocks` + `brand_id` + `toolbar`, as `principal`, at
/// logical time `now`. Creates on a fresh id (owner = principal, visibility = private); updates an
/// existing one (owner-only). Panel-block refs are validated + stripped. Returns the persisted record.
#[allow(clippy::too_many_arguments)]
pub async fn report_save(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    title: &str,
    blocks: Vec<Block>,
    brand_id: &str,
    toolbar: Value,
    now: u64,
) -> Result<Report, ReportError> {
    authorize_report(principal, ws, "report.save")?;
    if id.is_empty() {
        return Err(ReportError::BadInput("empty report id".into()));
    }
    if blocks.len() > MAX_BLOCKS {
        return Err(ReportError::BadInput(format!(
            "too many blocks: {} (max {MAX_BLOCKS})",
            blocks.len()
        )));
    }

    // Validate + strip each panel block's ref (dangling → BadInput, echoed spec dropped). A
    // markdown/image block's default (empty-ref) cell passes through the shipped seam untouched.
    let mut normalized = Vec::with_capacity(blocks.len());
    for mut block in blocks {
        if block.kind == "panel" {
            let cell = std::mem::take(&mut block.cell);
            let mut stripped = crate::validate_and_strip_refs(store, principal, ws, vec![cell])
                .await
                .map_err(ReportError::BadInput)?;
            block.cell = stripped.pop().unwrap_or_default();
        }
        normalized.push(block);
    }

    // Preserve owner + visibility across an update; only the owner may update. A tombstoned record
    // is treated as absent — a save with that id resurrects it under the new owner (create).
    let (owner, visibility) = match read_report(store, ws, id).await?.filter(|r| !r.deleted) {
        Some(existing) => {
            if existing.owner != principal.owner_sub() {
                return Err(ReportError::Denied);
            }
            (existing.owner, existing.visibility)
        }
        None => (principal.owner_sub().to_string(), Visibility::Private),
    };

    let report = Report {
        id: id.to_string(),
        title: title.to_string(),
        owner,
        visibility,
        blocks: normalized,
        brand_id: brand_id.to_string(),
        toolbar,
        schema_version: SCHEMA_VERSION,
        updated_ts: now,
        deleted: false,
    };
    write_report(store, ws, &report).await?;
    Ok(report)
}
