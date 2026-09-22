//! `subscribe_insight_events` — the live-feed motion read for `insight.watch` (insights umbrella
//! scope). Gates `mcp:insight.watch:call` (workspace-first) before declaring any bus interest, then
//! subscribes the workspace-scoped subject `ws/{ws}/insight/events` (raise/ack/resolve events). The
//! durable list is `insight.list`'s job; this is the "watch it grow" half (§3.3). The gateway SSE
//! route wraps the returned subscription for the browser.

use lb_auth::Principal;
use lb_bus::{subscribe, Bus, Subscription};
use lb_insights::RaiseEvent;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::InsightSvcError;

/// A live insight-events subscription — deserializes each bus payload back into a [`RaiseEvent`].
pub struct InsightWatch {
    inner: Subscription,
    /// Entity-scoped data: a restricted subscriber's `(tag, entity ids)` and the store to check each
    /// event's insight against. `None` = unrestricted, every event passes (unchanged behaviour).
    limit: Option<(Store, String, String, std::collections::BTreeSet<String>)>,
}

impl InsightWatch {
    /// Await the next insight event. `None` once the subscription closes; a malformed payload is
    /// skipped (never stalls the stream).
    pub async fn recv(&self) -> Option<RaiseEvent> {
        loop {
            let bytes = self.inner.recv().await?;
            let ev = match serde_json::from_slice::<RaiseEvent>(&bytes) {
                Ok(ev) => ev,
                Err(_) => continue,
            };
            // An event for an insight outside the subscriber's entities is dropped — its id and
            // dedup key are not theirs to see. An unreadable insight is dropped too (fail closed).
            if let Some((store, ws, tag, ids)) = &self.limit {
                let keep = match lb_insights::get(store, ws, &ev.id).await {
                    Ok(Some(i)) => i.tags.get(tag).is_some_and(|v| ids.contains(v)),
                    _ => false,
                };
                if !keep {
                    continue;
                }
            }
            return Some(ev);
        }
    }
}

/// Subscribe to workspace `ws`'s insight events as `principal`. Denies (opaque) without
/// `mcp:insight.watch:call` or across workspaces (the subject is ws-scoped — no cross-ws leak).
pub async fn subscribe_insight_events(
    bus: &Bus,
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<InsightWatch, InsightSvcError> {
    authorize_tool(principal, ws, "insight.watch").map_err(|_| InsightSvcError::Denied)?;
    let limit = super::entity_filter::entity_limit(store, principal, ws)
        .await?
        .map(|(tag, ids)| (store.clone(), ws.to_string(), tag, ids));
    // Same relative key the raise path publishes on (`lb_bus::publish(bus, ws, "insight/events")`
    // → `ws/{ws}/insight/events`).
    let inner = subscribe(bus, ws, "insight/events")
        .await
        .map_err(|e| InsightSvcError::Store(e.to_string()))?;
    Ok(InsightWatch { inner, limit })
}
