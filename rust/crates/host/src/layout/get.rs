//! `layout.get` — read the caller's OWN saved layout for a surface. Always keyed by the
//! authenticated principal's `sub` (member-owned; never a body field). Absent → a default
//! [`UiLayout`] (empty `model` — the client renders its built-in layout). Member-level, gated
//! `mcp:layout.get:call` (workspace-first, then the cap).

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::LayoutError;
use super::model::UiLayout;
use super::store::read_layout;

/// Read the caller's own layout for `surface`. Member-level.
pub async fn layout_get(
    store: &Store,
    principal: &Principal,
    ws: &str,
    surface: &str,
) -> Result<UiLayout, LayoutError> {
    authorize_tool(principal, ws, "layout.get").map_err(|_| LayoutError::Denied)?;
    if surface.is_empty() {
        return Err(LayoutError::BadInput("surface must not be empty".into()));
    }
    Ok(read_layout(store, ws, principal.sub(), surface)
        .await?
        .unwrap_or(UiLayout {
            surface: surface.to_string(),
            ..UiLayout::default()
        }))
}
