//! `telemetry.tail` — the **live feed** the console watches scroll (telemetry-console scope). The
//! right shape for "watch logs" is a `watch` over the ws-walled bus subject + a catch-up snapshot,
//! **not** polling `telemetry.query` on a timer (the scope says so). Mirrors `watch_run` /
//! `subscribe_channel`: a late join gets a snapshot of the recent ring, then the live motion the
//! `SurrealCappedLayer` mirrors onto the tail subject right after each capped insert.
//!
//! Authorization is `mcp:telemetry.read:call` through the shared chokepoint, workspace-first: a
//! ws-B principal can neither authorize for ws-A nor (structurally) subscribe to ws-A's subject
//! (`lb_bus` walls it under `ws/{id}/`). The snapshot is read from `ws`'s namespace only.

use lb_auth::Principal;
use lb_bus::{subscribe, Bus, Subscription};
use lb_store::Store;
use lb_telemetry::{TABLE, TAIL_SUBJECT};

use super::authorize::authorize_telemetry;
use super::error::TelemetrySvcError;

/// The catch-up snapshot a tail receives on attach: the recent ring (newest-first), bounded.
pub struct TailSnapshot {
    pub rows: Vec<serde_json::Value>,
}

/// The live telemetry subscription wrapper. `recv` yields the next mirrored telemetry row (the JSON
/// the publisher sent); `None` once the subscription closes. Mirrors `RunEventSub`/`BusSub`, typed
/// to "a telemetry row" (an opaque JSON value here — the SSE route emits it verbatim).
pub struct TailSub {
    inner: Subscription,
}

impl TailSub {
    /// Await the next live telemetry row (raw JSON bytes the Layer published). `None` once closed.
    pub async fn recv(&self) -> Option<Vec<u8>> {
        self.inner.recv().await
    }
}

/// Begin tailing `ws`'s telemetry as `principal`. Gated `mcp:telemetry.read:call` (opaque deny).
/// Returns the catch-up snapshot (recent rows) + the live ws-walled subscription. Subscribe BEFORE
/// reading the snapshot so no live row slips through the gap (a row arriving during the snapshot
/// read is buffered by the subscription; one overlap is benign — the UI keys rows by `seq`).
pub async fn telemetry_tail(
    store: &Store,
    bus: &Bus,
    principal: &Principal,
    ws: &str,
    snapshot_limit: usize,
) -> Result<(TailSnapshot, TailSub), TelemetrySvcError> {
    authorize_telemetry(principal, ws, "telemetry.tail")?;

    // Subscribe first (workspace-walled by lb_bus — a ws-B sub cannot reach ws-A's subject).
    let inner = subscribe(bus, ws, TAIL_SUBJECT)
        .await
        .map_err(|e| TelemetrySvcError::Store(lb_store::StoreError::Backend(e.to_string())))?;

    // Catch-up snapshot from THIS workspace's ring, newest-first, bounded.
    let n = snapshot_limit.clamp(1, super::filter::MAX_PAGE);
    let sql = format!(
        "SELECT seq, level, ws, actor, tool, source, trace_id, outcome, ts, msg, fields \
         FROM type::table($tb) WHERE ws = $ws ORDER BY seq DESC LIMIT {n}"
    );
    let mut resp = store
        .query_ws(
            ws,
            &sql,
            vec![
                ("tb".into(), serde_json::Value::String(TABLE.to_string())),
                ("ws".into(), serde_json::Value::String(ws.to_string())),
            ],
        )
        .await?;
    let rows: Vec<serde_json::Value> = resp
        .take(0)
        .map_err(|e| TelemetrySvcError::Store(lb_store::StoreError::Decode(e.to_string())))?;

    Ok((TailSnapshot { rows }, TailSub { inner }))
}
