//! The COV pump task (slice-6): `subscribe_cov` → decode each `CovEvent` → `frame::encode` → one
//! `ingest.write` sample onto the watch's series. Runs until the task is aborted (last subscriber gone,
//! or `appliance.remove`), reconnecting on a CE WS drop with bounded backoff so a subscriber sees a gap
//! rather than a dead stream.
//!
//! One responsibility: pump one series. The registry (`super::WatchRegistry`) owns arm/disarm; this file
//! owns the read→encode→write loop + reconnect. Motion is fire-and-forget (rule 3): a failed frame write
//! is dropped and the loop continues — the durable authority is CE, not this feed.

use std::time::Duration;

use futures::StreamExt;
use rubix_ce::{ControlEngine, CovScope};
use serde_json::json;

use super::frame;
use crate::host::HostCtx;

/// Backoff bounds for CE WS reconnect (mirror `ws.ts`'s STABLE_MS idea): start small, cap so a flapping
/// engine does not hot-loop.
const BACKOFF_MIN: Duration = Duration::from_millis(200);
const BACKOFF_MAX: Duration = Duration::from_secs(5);

/// Run the pump for `series` until aborted. Each decoded event becomes one `ingest.write` sample whose
/// `payload` is the frame JSON; `seq` is a monotonic per-series counter (the ingest dedup key), `ts` the
/// frame's own timestamp. On stream end/error the pump re-subscribes with bounded backoff.
pub async fn run(
    host: HostCtx,
    engine: std::sync::Arc<dyn ControlEngine>,
    series: String,
    scope: CovScope,
) {
    let mut seq: u64 = 0;
    let mut backoff = BACKOFF_MIN;

    loop {
        let mut stream = match engine.subscribe_cov(scope.clone()).await {
            Ok(s) => s,
            Err(_) => {
                // Could not open the CE WS — back off and retry (a gap, not a dead stream).
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(BACKOFF_MAX);
                continue;
            }
        };
        backoff = BACKOFF_MIN; // A successful subscribe resets the backoff.

        // Drain the stream until it ends (WS drop) or yields a decode error, then reconnect.
        while let Some(item) = stream.next().await {
            let event = match item {
                Ok(ev) => ev,
                Err(_) => continue, // A single decode error is skipped; the stream stays live.
            };
            let payload = frame::encode(&event);
            write_frame(&host, &series, seq, &payload).await;
            seq += 1;
        }
        // Stream ended (the engine closed the WS). Reconnect after a short backoff.
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(BACKOFF_MAX);
    }
}

/// Write one frame onto `series` via the `ingest.write` host callback. Fire-and-forget: a failure is
/// dropped (motion, §3.3) — the pump never dies on a transient host hiccup.
async fn write_frame(host: &HostCtx, series: &str, seq: u64, payload: &serde_json::Value) {
    let sample = json!({
        "series": series,
        // Producer is stamped host-side from the sidecar's ws-scoped token; a placeholder keeps the
        // `Sample` field present without asserting an identity (the ROS sink idiom).
        "producer": "",
        "ts": now_ms(),
        "seq": seq,
        "payload": payload,
    });
    let _ = host
        .client()
        .call_tool("ingest.write", json!({ "samples": [sample] }))
        .await;
}

/// Wall-clock ms for the sample envelope `ts` (data, not an ordering key — `seq` orders). Mirrors
/// `serve::now_ts`; the sidecar is an edge process with no clock-free core contract of its own.
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
