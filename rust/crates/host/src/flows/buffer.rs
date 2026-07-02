//! The durable **bounded accumulator** for the buffering stateful nodes (`batch` / `unique`-stream),
//! data-nodes Tier B. One record per node — `flow_node_buffer:{ws}:{flow}:{node}` — holding a capped
//! list of items across firings; it survives a restart (the Tier B two-firing + restart-parity
//! property) and is workspace-walled by the store namespace (the mandatory isolation guarantee).
//!
//! ## The one storage addition (data-nodes Risk 3, resolving Open Q3)
//!
//! A `batch` that never reaches its count, or a `unique` stream that sees unbounded distinct keys,
//! must not grow without bound. The bound is [`BATCH_MAX`]; the overflow policy is **force-release**
//! (return the buffer and clear it) rather than silent drop — a bounded buffer that never loses data
//! (the plc-reliability capped-ring precedent, but count-triggered, not a background reaper).
//!
//! ## Concurrency
//!
//! A buffer is a read-modify-write (unlike the counter's server-side atomic add), so same-node
//! firings are serialized by a per-`{ws}:{flow}:{node}` async lock — the `run_store` seed-lock /
//! store `key_lock` precedent. A flow has one owner node (spine Decision 10), so this in-process
//! lock is the whole contention story; different nodes never contend.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use lb_store::{read, write, Store};
use serde_json::{json, Value};
use tokio::sync::Mutex as AsyncMutex;

use super::record::{node_scoped_id, FLOW_NODE_BUFFER_TABLE};

/// The hard bound on a node's buffer (Open Q3). A `batch`/`unique` accumulator can never exceed this;
/// reaching it **force-releases** (below). Generous enough for real batching, small enough that a
/// runaway sequence can't exhaust memory.
pub const BATCH_MAX: usize = 1000;

/// Per-`{ws}:{flow}:{node}` lock serializing a buffer's read-modify-write.
fn buffer_lock(ws: &str, flow: &str, node: &str) -> Arc<AsyncMutex<()>> {
    static LOCKS: OnceLock<Mutex<HashMap<String, Arc<AsyncMutex<()>>>>> = OnceLock::new();
    let map = LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let key = format!("{ws}\u{1}{flow}\u{1}{node}");
    let mut guard = map.lock().expect("flows buffer-lock map poisoned");
    guard
        .entry(key)
        .or_insert_with(|| Arc::new(AsyncMutex::new(())))
        .clone()
}

/// Read the current buffered items (empty when absent). Used by tests + the release path.
pub async fn read_items(
    store: &Store,
    ws: &str,
    flow: &str,
    node: &str,
) -> Result<Vec<Value>, String> {
    let id = node_scoped_id(flow, node);
    match read(store, ws, FLOW_NODE_BUFFER_TABLE, &id)
        .await
        .map_err(|e| e.to_string())?
    {
        Some(v) => Ok(v
            .get("items")
            .and_then(|i| i.as_array())
            .cloned()
            .unwrap_or_default()),
        None => Ok(Vec::new()),
    }
}

/// Persist the buffered items (overwrites). `now` stamps the record for observability.
async fn write_items(
    store: &Store,
    ws: &str,
    flow: &str,
    node: &str,
    items: &[Value],
    now: u64,
) -> Result<(), String> {
    let id = node_scoped_id(flow, node);
    write(
        store,
        ws,
        FLOW_NODE_BUFFER_TABLE,
        &id,
        &json!({ "items": items, "ts": now }),
    )
    .await
    .map_err(|e| e.to_string())
}

/// The outcome of appending one payload to a `batch` buffer.
pub struct BatchStep {
    /// The items to emit as the grouped array, or `None` if the batch has not yet reached its
    /// boundary (the node **suppresses** — emits nothing this firing).
    pub released: Option<Vec<Value>>,
}

/// Append `item` to a `batch` buffer and decide whether the batch releases. Releases (returns the
/// items + clears the buffer) when the buffer reaches `count`, or when it hits [`BATCH_MAX`]
/// (force-release, Q3). Otherwise buffers and suppresses. Locked read-modify-write.
pub async fn batch_append(
    store: &Store,
    ws: &str,
    flow: &str,
    node: &str,
    item: Value,
    count: usize,
    now: u64,
) -> Result<BatchStep, String> {
    let lock = buffer_lock(ws, flow, node);
    let _guard = lock.lock().await;
    let mut items = read_items(store, ws, flow, node).await?;
    items.push(item);
    let target = count.clamp(1, BATCH_MAX);
    if items.len() >= target || items.len() >= BATCH_MAX {
        // Release: hand back the group and clear the buffer for the next window.
        write_items(store, ws, flow, node, &[], now).await?;
        Ok(BatchStep {
            released: Some(items),
        })
    } else {
        write_items(store, ws, flow, node, &items, now).await?;
        Ok(BatchStep { released: None })
    }
}

/// Test a `unique`-stream key against the durable seen-set, inserting it if new. Returns `true` when
/// the key was **not** seen before (the message passes) and `false` when it is a duplicate (dropped).
/// The seen-set is capped at [`BATCH_MAX`] with drop-oldest — a very old key may re-pass after the
/// ring rolls (bounded memory over perfect infinite dedup, the honest capped-ring trade).
pub async fn unique_seen(
    store: &Store,
    ws: &str,
    flow: &str,
    node: &str,
    key: &Value,
    now: u64,
) -> Result<bool, String> {
    let lock = buffer_lock(ws, flow, node);
    let _guard = lock.lock().await;
    let mut items = read_items(store, ws, flow, node).await?;
    if items.iter().any(|k| k == key) {
        return Ok(false);
    }
    items.push(key.clone());
    // Drop-oldest to stay within the bound (a seen-set is a ring, not a batch to force-release).
    while items.len() > BATCH_MAX {
        items.remove(0);
    }
    write_items(store, ws, flow, node, &items, now).await?;
    Ok(true)
}
