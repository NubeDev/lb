//! `panel.save(id, title, spec)` — one idempotent UPSERT for create+update (library-panels scope,
//! "MCP surface"; a fresh id creates, an existing id updates — not two verbs). Synchronous (one small
//! spec record; not a job). Gated by `mcp:panel.save:call`.
//!
//! **Ownership on update:** a save against an existing panel is allowed only for its owner — a
//! non-owner with the save cap still cannot overwrite someone else's panel (mirrors `dashboard.save`).
//! Create stamps `owner = principal`; `visibility` is set via `panel.share`, so save **preserves** the
//! existing visibility (it never silently re-privatizes a shared panel).

use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_panel;
use super::bounds::check_spec_bounds;
use super::error::PanelError;
use super::model::{Panel, PanelSpec, Visibility, SCHEMA_VERSION};
use super::store::{read_panel, write_panel};

/// Upsert panel `id` in `ws` with `title` + `spec`, as `principal`, at logical time `now`. Creates on
/// a fresh id (owner = principal, visibility = private); updates an existing one (owner-only). Returns
/// the persisted record.
pub async fn panel_save(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    title: &str,
    spec: PanelSpec,
    now: u64,
) -> Result<Panel, PanelError> {
    authorize_panel(principal, ws, "panel.save")?;
    if id.is_empty() {
        return Err(PanelError::BadInput("empty panel id".into()));
    }
    // Same record-growth bounds `dashboard.save` applies to a cell — the host is the boundary.
    check_spec_bounds(&spec, id)?;

    // Preserve owner + visibility across an update; only the owner may update. A tombstoned record is
    // treated as absent — a save with that id resurrects it under the new owner (create).
    let (owner, visibility) = match read_panel(store, ws, id).await?.filter(|p| !p.deleted) {
        Some(existing) => {
            if existing.owner != principal.sub() {
                return Err(PanelError::Denied);
            }
            (existing.owner, existing.visibility)
        }
        None => (principal.sub().to_string(), Visibility::Private),
    };

    let panel = Panel {
        id: id.to_string(),
        title: title.to_string(),
        owner,
        visibility,
        spec,
        schema_version: SCHEMA_VERSION,
        updated_ts: now,
        deleted: false,
    };
    write_panel(store, ws, &panel).await?;
    Ok(panel)
}
