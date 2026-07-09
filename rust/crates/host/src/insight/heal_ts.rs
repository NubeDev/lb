//! `heal_insight_timestamps` — a one-shot boot migration that normalizes any insight/occurrence
//! `ts` stored in epoch SECONDS to the epoch MILLISECONDS the surface is defined in.
//!
//! Historical records raised through the gateway `rules/run` route landed with `gw.now()` (=
//! `as_secs()`) as their `ts` — the UI (`new Date(ts)`) then rendered them as Jan 1970. New raises
//! are normalized at the write ([`super::raise::normalize_ts`]); this heals the ones already on disk.
//!
//! **Idempotent by construction:** it only rewrites values in the epoch-seconds band `[1e9, 1e12)`
//! (a real millis clock is ≥ `1e12`, a tiny logical/test clock is < `1e9`), and a scaled value lands
//! ≥ `1e12` — so a second run touches nothing. Best-effort per workspace: a scan/write hiccup logs
//! and is skipped, never blocking boot.

use lb_store::Store;
use serde_json::Value;

use super::raise::{TS_MILLIS_MIN, TS_SECONDS_MIN};

/// The insight parent table + its two timestamp columns, and the occurrence ring table + its one.
/// Kept local (not new public consts) — this migration is the only reader of the raw column names.
const INSIGHT_TABLE: &str = "insight";
const OCC_TABLE: &str = "insight_occ";

/// Scan `ws` for insight parents (`first_ts`/`last_ts`) and occurrence rows (`ts`) stamped in the
/// epoch-seconds band and rewrite them ×1000. Returns the number of rows updated (for the boot log).
pub async fn heal_insight_timestamps(store: &Store, ws: &str) -> u64 {
    let mut fixed = 0u64;
    // Parents are stored under a `data` envelope (`lb_store::write`), so their columns live at
    // `data.first_ts`/`data.last_ts`. Occurrence rows are written FLAT by `capped_insert` (no
    // envelope), so their timestamp is a top-level `ts`. Scale each, guarded to the seconds band so
    // a re-run (or an already-millis record) is a no-op.
    for col in ["data.first_ts", "data.last_ts"] {
        fixed += scale_column(store, ws, INSIGHT_TABLE, col).await;
    }
    fixed += scale_column(store, ws, OCC_TABLE, "ts").await;
    if fixed > 0 {
        tracing::info!(%ws, fixed, "healed insight timestamps (epoch-seconds → millis)");
    }
    fixed
}

/// `UPDATE <table> SET <col> = <col> * 1000 WHERE <col> IN [1e9, 1e12)` — the seconds-band guard
/// makes it idempotent. Best-effort: a store error logs and returns 0 (never blocks boot).
async fn scale_column(store: &Store, ws: &str, table: &str, col: &str) -> u64 {
    // The column name is a fixed literal (never user input), so interpolating it into the statement
    // is safe here; the band bounds ride bound params. `RETURN VALUE {col}` yields ONLY the scaled
    // scalar per updated row — NOT the whole record (whose `id` is a Surreal thing that won't
    // deserialize into JSON) — so `.take()` decodes cleanly and its length is the updated-row count.
    let sql = format!(
        "UPDATE type::table($tb) SET {col} = {col} * 1000 \
         WHERE {col} >= $lo AND {col} < $hi RETURN VALUE {col}"
    );
    let res = store
        .query_ws(
            ws,
            &sql,
            vec![
                ("tb".into(), Value::String(table.to_string())),
                ("lo".into(), Value::Number(TS_SECONDS_MIN.into())),
                ("hi".into(), Value::Number(TS_MILLIS_MIN.into())),
            ],
        )
        .await;
    match res {
        Ok(mut resp) => resp
            .take::<Vec<Value>>(0)
            .map(|rows| rows.len() as u64)
            .unwrap_or_else(|e| {
                tracing::warn!(%ws, table, col, error = %format!("{e:?}"), "insight ts heal decode");
                0
            }),
        Err(e) => {
            tracing::warn!(%ws, table, col, error = %format!("{e:?}"), "insight ts heal skipped");
            0
        }
    }
}
