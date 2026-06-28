//! Write two records — a **domain change** and its **outbox effect** — in ONE SurrealDB
//! transaction (the transactional-outbox pattern, README §6.10, outbox scope).
//!
//! This is the load-bearing seam of the must-deliver outbox: either both upserts commit or
//! neither does. There is no window where the domain change is durable but the intent-to-deliver
//! is lost (an orphaned change), nor where the effect is scheduled for a change that never landed
//! (a phantom delivery). A single `BEGIN … COMMIT TRANSACTION` around both upserts is the whole
//! mechanism — SurrealDB rolls the transaction back as a unit if either statement fails.
//!
//! Both records land in the SAME workspace namespace (selected from `ws` first), so the hard wall
//! holds for the transaction exactly as for a single write (README §7). Caller is expected to have
//! passed `caps::check` — this is the raw verb, not the authorization point.

use serde_json::Value;

use crate::open::{Store, StoreError};
use crate::record::FIRST_REV;

/// One record to upsert: its table, id, and host JSON value.
pub struct Upsert<'a> {
    pub table: &'a str,
    pub id: &'a str,
    pub value: &'a Value,
}

/// Atomically upsert `change` and `effect` into workspace `ws` in one transaction. Both wrap the
/// host JSON under `data` (the same envelope as `write`) and bump a monotonic `rev` (the same
/// envelope `write` uses), so the read path is identical. If either upsert fails the whole
/// transaction rolls back — the durability guarantee the outbox relies on.
pub async fn write_tx(
    store: &Store,
    ws: &str,
    change: &Upsert<'_>,
    effect: &Upsert<'_>,
) -> Result<(), StoreError> {
    let db = store.use_ws(ws).await?;
    db.query(
        "BEGIN TRANSACTION;
         UPSERT type::thing($ct, $cid) CONTENT { \
            data: $cdata, \
            rev: (type::thing($ct, $cid).rev ?? ($first - 1)) + 1 \
         } RETURN NONE;
         UPSERT type::thing($et, $eid) CONTENT { \
            data: $edata, \
            rev: (type::thing($et, $eid).rev ?? ($first - 1)) + 1 \
         } RETURN NONE;
         COMMIT TRANSACTION;",
    )
    .bind(("ct", change.table.to_string()))
    .bind(("cid", change.id.to_string()))
    .bind(("cdata", change.value.clone()))
    .bind(("et", effect.table.to_string()))
    .bind(("eid", effect.id.to_string()))
    .bind(("edata", effect.value.clone()))
    .bind(("first", FIRST_REV))
    .await?
    .check()?;
    Ok(())
}
