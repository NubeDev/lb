//! Post a message to a channel — the write verb of the messaging slice.
//!
//! The flow is the state-vs-motion split made concrete (§3.3):
//!   1. authorize (`bus:chan/{cid}:pub`, workspace-first) — capability-first, before anything;
//!   2. persist the normalized item to the store via the inbox (STATE — survives a restart);
//!   3. publish the same item onto the bus (MOTION — subscribers see it appear in real time).
//!
//! Persist-before-publish on purpose: the durable record is the source of truth, the bus push
//! is the live echo. A subscriber that missed the push recovers the message from `history`;
//! the inverse (publish first, persist later) could echo a message that never durably landed.
//!
//! The same persist-then-publish is shared by the **channel query worker** ([`deliver`]), which the
//! host posts `query_result` / `query_error` items through under a system identity (the worker is
//! host-internal, so it does not re-run the channel `pub` gate — it IS the host posting a result,
//! like any system message). One owner of "persist a channel item + echo it on the bus" — no drift.

use lb_auth::Principal;
use lb_bus::Bus;
use lb_caps::Action;
use lb_inbox::Item;
use lb_store::Store;

use super::authorize::authorize;
use super::error::ChannelError;
use crate::boot::Node;

/// Post `item` to channel `cid` in workspace `ws` as `principal`. The item's `channel` is set
/// to `cid` (the caller need not repeat it). Returns once persisted *and* published.
///
/// Takes the [`Node`] (not just `store`+`bus`) so the inline query worker can run `federation.query`
/// (channels-query-charts scope) — the post path is where a `kind:"query"` item is answered.
pub async fn post(
    node: &Node,
    principal: &Principal,
    ws: &str,
    cid: &str,
    mut item: Item,
) -> Result<Item, ChannelError> {
    authorize(principal, ws, cid, Action::Pub)?;
    item.channel = cid.to_string();
    let delivered = deliver(&node.store, &node.bus, ws, cid, item).await?;

    // INLINE query worker (channels-query-charts scope): a `kind:"query"` item is the request; the
    // host answers it right here in the post path (one item → one execution, idempotent by
    // construction — no bus-redelivery dedup needed). Only `kind:"query"` triggers work; the
    // worker's own result/error items do NOT (the explicit re-entrancy guard). A failure inside the
    // worker NEVER fails the originating post — the query item already durably landed; the worst
    // case is a follow-up `query_error` item (or none if the worker itself could not persist).
    super::query_worker::run_if_query(node, principal, ws, cid, &delivered).await;

    Ok(delivered)
}

/// Persist `item` to channel `cid`'s inbox (STATE) and publish it on the bus (MOTION), after
/// best-effort register-on-post. Shared by the member-facing [`post`] and the host-internal query
/// worker. `item.channel` is forced to `cid`. The item is returned with the channel filled in.
pub(crate) async fn deliver(
    store: &Store,
    bus: &Bus,
    ws: &str,
    cid: &str,
    mut item: Item,
) -> Result<Item, ChannelError> {
    use lb_bus::publish;

    item.channel = cid.to_string();

    // STATE: durable first.
    lb_inbox::record(store, ws, &item).await?;

    // REGISTRY: make the channel listable (create-on-first-post). Best-effort and additive — a
    // registry hiccup must never fail a posted message (the durable item is the source of truth),
    // so the result is intentionally ignored. Idempotent: re-posting upserts the same row.
    let _ = crate::channel_registry::register_on_post(store, ws, cid, &item.author, item.ts).await;

    // MOTION: live echo. Serialized item JSON is the payload; subscribers deserialize it.
    let payload = serde_json::to_vec(&item)
        .map_err(|e| ChannelError::Store(lb_store::StoreError::Decode(e.to_string())))?;
    publish(bus, ws, &super::key::msg_key(cid, &item.id), &payload).await?;

    Ok(item)
}
