//! List a workspace's assets owned by a principal, oldest→newest. Tombstoned (deleted) assets
//! are skipped — a delete is a tombstone upsert, not a vanished row. Mirrors `doc::list`.

use lb_store::{list as store_list, Store, StoreError};

use super::model::{Asset, TOMBSTONE};
use super::TABLE;

/// Return every live asset in `ws` owned by `owner`, ordered by `ts` ascending. Empty if none
/// — never another workspace's assets (§7).
pub async fn list_assets(store: &Store, ws: &str, owner: &str) -> Result<Vec<Asset>, StoreError> {
    let rows = store_list(store, ws, TABLE, "owner", owner).await?;
    let mut assets: Vec<Asset> = Vec::new();
    for v in rows {
        if v.get("id").and_then(|k| k.as_str()) == Some(TOMBSTONE) {
            continue;
        }
        let a: Asset = serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string()))?;
        assets.push(a);
    }
    assets.sort_by_key(|a| a.ts);
    Ok(assets)
}
