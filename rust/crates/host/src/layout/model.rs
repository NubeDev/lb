//! The [`UiLayout`] record — one member's saved layout for one dockable surface. The `model` is the
//! client's layout JSON (for Data Studio: the FlexLayout model incl. per-tab draft configs) — opaque
//! to the host, bounded, never interpreted server-side.

use serde::{Deserialize, Serialize};

/// The `ui_layout` table (keyed `[ws, user, surface]` — see `store::layout_id`).
pub const TABLE: &str = "ui_layout";

/// The serialized `model` size cap. A FlexLayout model with a dozen tabs + draft cells is a few KB;
/// 256 KB is generous headroom while still refusing an unbounded blob (the host is the authority —
/// reject, don't silently truncate).
pub const MAX_LAYOUT_BYTES: usize = 256 * 1024;

/// One member's saved layout for one surface. Absent = the client renders its default layout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct UiLayout {
    /// The surface key the layout belongs to (e.g. `data-studio`) — opaque data to the host.
    #[serde(default)]
    pub surface: String,
    /// The client's layout JSON (opaque; bounded by [`MAX_LAYOUT_BYTES`]). `null` = cleared.
    #[serde(default)]
    pub model: serde_json::Value,
    pub updated_ts: u64,
}
