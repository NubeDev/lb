//! `nav.save(id, title, items)` — one idempotent UPSERT for create+update (nav scope, "MCP surface";
//! a fresh id creates, an existing id updates — not two verbs). Synchronous (one small menu record).
//! Gated by `mcp:nav.save:call`.
//!
//! **Ownership on update:** a save against an existing nav is allowed only for its owner — a non-owner
//! with the save cap still cannot overwrite someone else's nav (mirrors `dashboard_save`). Create
//! stamps `owner = principal`; `visibility` is set via `nav.share`, so save **preserves** the existing
//! visibility (it never silently re-privatizes a shared nav). The nav carries NO caps — an item can
//! never widen reach (nav scope, "No per-entry cap authoring"); `items[]` is bounded here.

use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_nav;
use super::bounds::check_items;
use super::error::NavError;
use super::model::{Nav, NavItem, Visibility, SCHEMA_VERSION};
use super::store::{read_nav, write_nav};

/// Upsert nav `id` in `ws` with `title` + `items`, as `principal`, at logical time `now`. Creates on
/// a fresh id (owner = principal, visibility = private); updates an existing one (owner-only). Returns
/// the persisted record.
pub async fn nav_save(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    title: &str,
    items: Vec<NavItem>,
    now: u64,
) -> Result<Nav, NavError> {
    authorize_nav(principal, ws, "nav.save")?;
    if id.is_empty() {
        return Err(NavError::BadInput("empty nav id".into()));
    }
    check_items(&items)?;

    // Preserve owner + visibility across an update; only the owner may update. A tombstoned record is
    // treated as absent — a save with that id resurrects it under the new owner (create).
    let (owner, visibility) = match read_nav(store, ws, id).await?.filter(|n| !n.deleted) {
        Some(existing) => {
            if existing.owner != principal.sub() {
                return Err(NavError::Denied);
            }
            (existing.owner, existing.visibility)
        }
        None => (principal.sub().to_string(), Visibility::Private),
    };

    let nav = Nav {
        id: id.to_string(),
        title: title.to_string(),
        owner,
        visibility,
        items,
        schema_version: SCHEMA_VERSION,
        updated_ts: now,
        deleted: false,
    };
    write_nav(store, ws, &nav).await?;
    Ok(nav)
}
