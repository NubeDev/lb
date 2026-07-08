//! The raw store read/write for [`Nav`] / [`NavPref`] / the workspace-default pointer — the
//! (de)serialization seam between the typed models and the generic `lb_store` `data`-envelope. Kept
//! in one file so the table names + the envelope shape have a single owner (FILE-LAYOUT). No
//! authorization here — the verbs gate first.

use lb_store::{read, scan, write, Store, StoreError};

use super::model::{Nav, NavHidden, NavPref, DEFAULT_TABLE, HIDDEN_TABLE, PREF_TABLE, TABLE};

/// The largest roster a single `nav.list` returns (one scan page). A workspace with more navs than
/// this is a named follow-up (paged roster) — stated, not silently truncated.
pub const MAX_NAVS: usize = lb_store::MAX_SCAN_LIMIT;

/// The `nav_pref` / `workspace_nav_default` composite id from a `[ws, key]` pair. `lb_store` already
/// namespaces every key by workspace, so the record id only needs the second axis (the user, or a
/// constant for the ws-wide default). Kept one place so the id shape has a single owner.
fn pref_id(user: &str) -> String {
    user.to_string()
}

/// The constant record id of the one workspace-default pointer per workspace.
const DEFAULT_ID: &str = "default";

/// The constant record id of the one hidden-set per workspace (`nav_hidden:[ws]`).
const HIDDEN_ID: &str = "hidden";

/// Read `nav:{id}` in `ws`. `None` if absent in this namespace (the hard wall) — a tombstoned record
/// still deserializes (callers treat `deleted` as absent).
pub async fn read_nav(store: &Store, ws: &str, id: &str) -> Result<Option<Nav>, StoreError> {
    match read(store, ws, TABLE, id).await? {
        Some(v) => {
            let n: Nav =
                serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string()))?;
            Ok(Some(n))
        }
        None => Ok(None),
    }
}

/// UPSERT `nav` at `nav:{id}` in `ws` (create+update; idempotent on the id).
pub async fn write_nav(store: &Store, ws: &str, n: &Nav) -> Result<(), StoreError> {
    let value = serde_json::to_value(n).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, TABLE, &n.id, &value).await
}

/// Scan up to [`MAX_NAVS`] navs in `ws` (one page, id-ordered). The roster read — the caller then
/// filters by gate-3 visibility and drops tombstones.
pub async fn scan_navs(store: &Store, ws: &str) -> Result<Vec<Nav>, StoreError> {
    let page = scan(store, ws, TABLE, MAX_NAVS, None).await?;
    let mut out = Vec::with_capacity(page.rows.len());
    for row in page.rows {
        // `lb_store::write` wraps records in a `{ data: ... }` envelope; `scan` returns the whole
        // record, so unwrap the envelope to get the nav (same shape `scan_dashboards` unwraps).
        let inner = match row.data {
            serde_json::Value::Object(mut o) => o.remove("data").unwrap_or(serde_json::Value::Null),
            other => other,
        };
        let n: Nav =
            serde_json::from_value(inner).map_err(|e| StoreError::Decode(e.to_string()))?;
        out.push(n);
    }
    Ok(out)
}

/// Read the member's active-pick record (`nav_pref:[ws, user]`). `None` when they've never picked.
pub async fn read_pref(store: &Store, ws: &str, user: &str) -> Result<Option<NavPref>, StoreError> {
    match read(store, ws, PREF_TABLE, &pref_id(user)).await? {
        Some(v) => {
            let p: NavPref =
                serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string()))?;
            Ok(Some(p))
        }
        None => Ok(None),
    }
}

/// UPSERT the member's active-pick record. Idempotent on `[ws, user]` (LWW).
pub async fn write_pref(
    store: &Store,
    ws: &str,
    user: &str,
    pref: &NavPref,
) -> Result<(), StoreError> {
    let value = serde_json::to_value(pref).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, PREF_TABLE, &pref_id(user), &value).await
}

/// Read the workspace hidden-set (`nav_hidden:[ws]`). `None` when the admin never set one — the
/// resolver treats that as an empty set (nothing hidden).
pub async fn read_hidden(store: &Store, ws: &str) -> Result<Option<NavHidden>, StoreError> {
    match read(store, ws, HIDDEN_TABLE, HIDDEN_ID).await? {
        Some(v) => {
            let h: NavHidden =
                serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string()))?;
            Ok(Some(h))
        }
        None => Ok(None),
    }
}

/// UPSERT the workspace hidden-set. Idempotent on the one `[ws]` record (LWW; an empty `hidden`
/// clears it — a tombstone shape, like the default pointer).
pub async fn write_hidden(store: &Store, ws: &str, h: &NavHidden) -> Result<(), StoreError> {
    let value = serde_json::to_value(h).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, HIDDEN_TABLE, HIDDEN_ID, &value).await
}

/// Read the workspace-default nav id (`workspace_nav_default:[ws]`). `None` when none is set.
pub async fn read_default(store: &Store, ws: &str) -> Result<Option<String>, StoreError> {
    match read(store, ws, DEFAULT_TABLE, DEFAULT_ID).await? {
        Some(v) => Ok(v
            .get("nav")
            .and_then(|x| x.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())),
        None => Ok(None),
    }
}

/// UPSERT the workspace-default nav pointer. An empty `nav_id` clears it (a tombstone shape).
pub async fn write_default(
    store: &Store,
    ws: &str,
    nav_id: &str,
    now: u64,
) -> Result<(), StoreError> {
    let value = serde_json::json!({ "nav": nav_id, "updated_ts": now });
    write(store, ws, DEFAULT_TABLE, DEFAULT_ID, &value).await
}
