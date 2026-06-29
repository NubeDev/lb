//! Delete a single item by `(channel, id)` from the durable inbox.
//!
//! The store erase is namespace-scoped, so a delete for workspace A can only ever touch A's
//! items (README §7). Authorization is the caller's job — run before this raw verb. Idempotent:
//! erasing an already-absent item is a no-op success.

use lb_store::{delete as erase, Store, StoreError};

use crate::record::{record_id, TABLE};

/// Erase the item at `(ws, channel, id)`. No-op (still `Ok`) if it is already absent.
pub async fn delete(store: &Store, ws: &str, channel: &str, id: &str) -> Result<(), StoreError> {
    erase(store, ws, TABLE, &record_id(channel, id)).await
}
