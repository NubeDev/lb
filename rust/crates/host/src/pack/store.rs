//! The receipt's persistence — the raw store read/write for a [`Receipt`], and nothing else. No
//! authorization here: the verbs gate first (the house pattern, cf. `dashboard/store.rs`).
//!
//! Receipts are FIRST-CLASS (pack-core-scope §"Receipts as records"): an internal table read by
//! `pack.list`/`pack.get`, NOT the public `store.*` verbs. The prototype had to write receipts with
//! `store.write` and read them back with `SELECT data FROM pack_receipts WHERE data.pack = …` — a
//! query shaped entirely by store envelope quirks (the `{data: …}` wrapper, a `thing` id that 502s
//! `SELECT *`). None of that survives the port: `lb_store::read` unwraps the envelope for us, the
//! record id is the pack name, and a reader gets a typed [`Receipt`].
//!
//! The workspace wall is the store's namespace (`ws` selects the SurrealDB namespace), so a
//! receipt written in one workspace is physically unreadable from another.

use lb_packs::Receipt;
use lb_store::{read, scan_all, write, Store, StoreError};

/// The receipt table. One row per pack per workspace, keyed by the pack name.
pub const TABLE: &str = "pack_receipt";

/// Read the receipt for `pack` in `ws`, if one exists.
pub async fn read_receipt(
    store: &Store,
    ws: &str,
    pack: &str,
) -> Result<Option<Receipt>, StoreError> {
    match read(store, ws, TABLE, pack).await? {
        Some(value) => Ok(Some(
            serde_json::from_value(value).map_err(|e| StoreError::Decode(e.to_string()))?,
        )),
        None => Ok(None),
    }
}

/// Upsert `receipt` in `ws`. Written atomically at the END of an apply — a partial apply still
/// writes its receipt, because the partial IS the recovery signal.
pub async fn write_receipt(store: &Store, ws: &str, receipt: &Receipt) -> Result<(), StoreError> {
    let value = serde_json::to_value(receipt).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, TABLE, &receipt.pack, &value).await
}

/// Every receipt in `ws`, for the roster read.
pub async fn scan_receipts(store: &Store, ws: &str) -> Result<Vec<Receipt>, StoreError> {
    let rows = scan_all(store, ws, TABLE).await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        // `read` unwraps the store's `{data, rev}` envelope; `scan` does NOT (it selects the whole
        // record), so `Row::data` here is the ENVELOPE, not the receipt. Unwrap the inner `data`
        // when it is there, and fall back to the bare value so a record written by any other path
        // still decodes.
        //
        // This asymmetry is load-bearing enough to spell out: getting it wrong made `pack.list`
        // return an empty roster for every workspace, silently, while `pack.get` worked perfectly —
        // because the failed decode was swallowed. Hence the `Err` arm below is LOUD.
        let value = row.data.get("data").cloned().unwrap_or(row.data);
        match serde_json::from_value::<Receipt>(value) {
            Ok(r) => out.push(r),
            // A receipt that will not decode is a corrupt record, not an absent one. Dropping it
            // silently would under-report what is applied to this workspace — exactly the failure
            // an operator cannot see. Say so, and keep going: one bad row must not blank the roster.
            Err(e) => tracing::warn!(
                target: "pack",
                ws,
                error = %e,
                "skipping an undecodable pack receipt — the roster is INCOMPLETE"
            ),
        }
    }
    out.sort_by(|a, b| a.pack.cmp(&b.pack));
    Ok(out)
}
