//! `layout.set` — upsert the caller's OWN layout for a surface at logical time `now` (the caller's
//! clock — determinism; never wall-clock in the verb). Always keyed by `principal.sub()` — a caller
//! cannot write another user's layout (the member-owned test). The model is opaque JSON, bounded by
//! [`super::model::MAX_LAYOUT_BYTES`] (reject, don't truncate). A `null` model clears the record's
//! content (the client falls back to its default layout). Member-level, gated `mcp:layout.set:call`.

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::LayoutError;
use super::model::{UiLayout, MAX_LAYOUT_BYTES};
use super::store::write_layout;

/// Upsert the caller's own layout for `surface`. Returns the stored record. Member-level.
pub async fn layout_set(
    store: &Store,
    principal: &Principal,
    ws: &str,
    surface: &str,
    model: serde_json::Value,
    now: u64,
) -> Result<UiLayout, LayoutError> {
    authorize_tool(principal, ws, "layout.set").map_err(|_| LayoutError::Denied)?;
    if surface.is_empty() {
        return Err(LayoutError::BadInput("surface must not be empty".into()));
    }
    let size = serde_json::to_vec(&model).map(|v| v.len()).unwrap_or(0);
    if size > MAX_LAYOUT_BYTES {
        return Err(LayoutError::BadInput(format!(
            "layout model is {size} bytes, exceeds cap {MAX_LAYOUT_BYTES}"
        )));
    }
    let layout = UiLayout {
        surface: surface.to_string(),
        model,
        updated_ts: now,
    };
    write_layout(store, ws, principal.sub(), &layout).await?;
    Ok(layout)
}
