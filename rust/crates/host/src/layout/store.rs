//! The raw store read/write for [`UiLayout`] — the (de)serialization seam between the typed model
//! and the generic `lb_store` `data`-envelope. No authorization here — the verbs gate first.

use lb_store::{read, write, Store, StoreError};

use super::model::{UiLayout, TABLE};

/// The `ui_layout` composite record id from `[user, surface]`. `lb_store` already namespaces every
/// key by workspace, so the id carries the remaining two axes. `\u{1f}` (unit separator) cannot
/// appear in a principal `sub` or a surface key, so the pair is unambiguous.
fn layout_id(user: &str, surface: &str) -> String {
    format!("{user}\u{1f}{surface}")
}

/// Read the member's layout for `surface`. `None` when they've never saved one.
pub async fn read_layout(
    store: &Store,
    ws: &str,
    user: &str,
    surface: &str,
) -> Result<Option<UiLayout>, StoreError> {
    match read(store, ws, TABLE, &layout_id(user, surface)).await? {
        Some(v) => {
            let l: UiLayout =
                serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string()))?;
            Ok(Some(l))
        }
        None => Ok(None),
    }
}

/// UPSERT the member's layout for `surface`. Idempotent on `[ws, user, surface]` (LWW).
pub async fn write_layout(
    store: &Store,
    ws: &str,
    user: &str,
    layout: &UiLayout,
) -> Result<(), StoreError> {
    let value = serde_json::to_value(layout).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, TABLE, &layout_id(user, &layout.surface), &value).await
}
