//! `subscribe_insight_events` — the live-feed motion read for `insight.watch` (insights umbrella
//! scope). Gates `mcp:insight.watch:call` (workspace-first) before declaring any bus interest, then
//! subscribes the workspace-scoped subject `ws/{ws}/insight/events` (raise/ack/resolve events). The
//! durable list is `insight.list`'s job; this is the "watch it grow" half (§3.3). The gateway SSE
//! route wraps the returned subscription for the browser.

use lb_auth::Principal;
use lb_bus::{subscribe, Bus, Subscription};
use lb_insights::RaiseEvent;
use lb_mcp::authorize_tool;

use super::error::InsightSvcError;

/// A live insight-events subscription — deserializes each bus payload back into a [`RaiseEvent`].
pub struct InsightWatch {
    inner: Subscription,
}

impl InsightWatch {
    /// Await the next insight event. `None` once the subscription closes; a malformed payload is
    /// skipped (never stalls the stream).
    pub async fn recv(&self) -> Option<RaiseEvent> {
        loop {
            let bytes = self.inner.recv().await?;
            match serde_json::from_slice::<RaiseEvent>(&bytes) {
                Ok(ev) => return Some(ev),
                Err(_) => continue,
            }
        }
    }
}

/// Subscribe to workspace `ws`'s insight events as `principal`. Denies (opaque) without
/// `mcp:insight.watch:call` or across workspaces (the subject is ws-scoped — no cross-ws leak).
pub async fn subscribe_insight_events(
    bus: &Bus,
    principal: &Principal,
    ws: &str,
) -> Result<InsightWatch, InsightSvcError> {
    authorize_tool(principal, ws, "insight.watch").map_err(|_| InsightSvcError::Denied)?;
    // Same relative key the raise path publishes on (`lb_bus::publish(bus, ws, "insight/events")`
    // → `ws/{ws}/insight/events`).
    let inner = subscribe(bus, ws, "insight/events")
        .await
        .map_err(|e| InsightSvcError::Store(e.to_string()))?;
    Ok(InsightWatch { inner })
}
