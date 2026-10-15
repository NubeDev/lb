//! `next_seq` — the workspace's running case number (`Case::seq`): 1, 2, 3 … in opening order.
//!
//! A case id is a ULID, unique and sortable but not something a person reads out on a call. The
//! number is the reference they can: one counter per workspace, taken once per case at open.
//!
//! **Atomic, through the store's `increment`.** Two cases opened at once each get their own number;
//! the add happens inside one UPSERT, never read-then-write in the host.
//!
//! **Backfill on first use.** A workspace that had cases before numbering existed has no counter
//! record. The host's case tick calls [`number_existing_cases`] so that happens soon after an upgrade
//! rather than waiting for a new case (on a busy estate most alerts re-fire into cases that already
//! exist, so a new open can be days away). Failing that, the first numbered open notices it, numbers every existing case in opening order
//! (oldest first, id as the tie-break), and sets the counter to the count, so the new case follows
//! the old ones. It runs under a per-workspace lock, so a second open waits rather than numbering
//! the same cases twice. The backfill writes `seq` only: `last_activity_ts` is left alone, because
//! numbering a case is not something happening to it.
//!
//! Numbers are never reused. A case that is later merged away keeps its number and the next case
//! takes the next one, so gaps are possible and expected.
//!
//! One responsibility: hand out the next case number.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use lb_store::{increment, read, write, Store};
use tokio::sync::Mutex as AsyncMutex;

use crate::case::TABLE;
use crate::error::CasesError;
use crate::list_read::read_cases;

/// The table the per-workspace counter lives in, and its one record.
pub const SEQ_TABLE: &str = "case_seq";
const SEQ_ID: &str = "case";

/// One lock per workspace, held across the "is there a counter yet?" check and the backfill.
fn ws_lock(ws: &str) -> Arc<AsyncMutex<()>> {
    static LOCKS: OnceLock<Mutex<HashMap<String, Arc<AsyncMutex<()>>>>> = OnceLock::new();
    let map = LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = map.lock().expect("case seq lock map poisoned");
    guard
        .entry(ws.to_string())
        .or_insert_with(|| Arc::new(AsyncMutex::new(())))
        .clone()
}

/// The next case number in `ws`, backfilling the workspace's existing cases first if it has never
/// numbered one.
pub(crate) async fn next_seq(store: &Store, ws: &str, ts: u64) -> Result<u64, CasesError> {
    let lock = ws_lock(ws);
    let _guard = lock.lock().await;
    if read(store, ws, SEQ_TABLE, SEQ_ID).await?.is_none() {
        backfill(store, ws, ts).await?;
    }
    let n = increment(store, ws, SEQ_TABLE, SEQ_ID, 1, false, ts).await?;
    Ok(n.max(1) as u64)
}

/// Number the workspace's existing cases if it has never numbered one, and return how many were
/// numbered. Once the counter exists this is one record read and `0`, so it is safe on a timer.
pub async fn number_existing_cases(store: &Store, ws: &str, ts: u64) -> Result<usize, CasesError> {
    let lock = ws_lock(ws);
    let _guard = lock.lock().await;
    if read(store, ws, SEQ_TABLE, SEQ_ID).await?.is_some() {
        return Ok(0);
    }
    backfill(store, ws, ts).await
}

/// Number every case in `ws` that has no `seq`, oldest first, and leave the counter at the count.
async fn backfill(store: &Store, ws: &str, ts: u64) -> Result<usize, CasesError> {
    let mut cases = read_cases(store, ws, true, None).await?;
    cases.retain(|c| c.seq.is_none());
    cases.sort_by(|a, b| a.opened_ts.cmp(&b.opened_ts).then_with(|| a.id.cmp(&b.id)));
    let total = cases.len() as i64;
    for (i, case) in cases.iter_mut().enumerate() {
        case.seq = Some(i as u64 + 1);
        let value = serde_json::to_value(&*case).map_err(CasesError::decode)?;
        write(store, ws, TABLE, &case.id, &value).await?;
    }
    // Set the counter even when there was nothing to number, so the check above passes from now on.
    increment(store, ws, SEQ_TABLE, SEQ_ID, total, true, ts).await?;
    Ok(cases.len())
}
