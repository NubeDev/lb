# Inbox and outbox scope

Status: scope. The inbox (normalized items) shipped at S2 with the messaging slice; the
transactional outbox (durable must-deliver) is deferred until a second node exists (S3+).

> Read with: `../../README.md` §6.10 (inbox/outbox), §3.3 (state vs motion), `../bus/bus-scope.md`
> (the bus is motion; the inbox is the durable state behind it).

---

## Goal

One **normalized item** shape that every source (a chat message, a job result, a system notice)
collapses into, so a single channel view / unread count / triage flow works across all of them.
The inbox is **state** (it lives in SurrealDB, behind the workspace wall); the bus moves a copy
as **motion**. The outbox is the durability backstop for *must-deliver* messages — not raw
pub/sub.

## What shipped in S2 (the inbox)

- `lb_inbox::Item` — `{ id, channel, author, body, ts }`. `id` is caller-supplied and stable, so
  re-delivering the same `(channel, id)` **upserts** one row (idempotent delivery). `ts` is a
  caller-injected logical timestamp (no wall-clock — testing §3 determinism).
- `record(store, ws, item)` — persist via `lb_store` into the workspace namespace, at
  `inbox:<channel>__<id>`.
- `list(store, ws, channel)` — every item in a channel, sorted by `ts` oldest→newest (the store
  `list` filters; the inbox owns the order key — see
  debugging/store/order-by-needs-selected-idiom.md).
- The host `channel` service is the capability chokepoint: `post` persists an item (state) then
  publishes it (motion); `history` reads the durable items. Authorization (`bus:chan/{cid}:{pub|
  sub}`) runs first.

## Non-goals (S2)

- The **transactional outbox** (must-deliver with at-least-once delivery + dedup on the receiver)
  — needs a second node to deliver *to*; lands with the multi-node slice (S3+). At S2 there is one
  node, so persist-before-publish + durable history already give recovery.
- Unread/triage/labels on items — derived views, added when an app needs them.
- Relays / cross-workspace forwarding — out of scope (the workspace wall is the point).

## How it fits the core

- **State vs motion (§3.3):** the inbox is the durable record; the bus is the live echo. `post`
  persists *before* it publishes, so a missed live push is always recoverable from `history`.
- **Workspace wall (§7):** items live in the workspace namespace; a `list` in workspace B
  physically cannot return workspace A's items (tested across store + inbox).
- **Capability-first (§3.5):** the raw `record`/`list` verbs assume the caller already passed
  `caps::check`; the host `channel` service is where the check lives — there is no unauthorized
  path to an item.
- **Idempotency:** stable `(channel, id)` keys make re-delivery safe, which is the precondition
  the must-deliver outbox will rely on (a receiver dedups on item id).

## Testing plan

- S2 (shipped): `inbox/inbox_test` — record+list ordered, idempotent re-record, channels
  independent within a workspace, **mandatory workspace-isolation** (B never sees A's items).
  Channel-level coverage in `host/messaging_*` (deny, isolation across all three surfaces).
- Outbox (S3+): write a must-deliver message offline on an edge, reconnect, assert at-least-once
  delivery with receiver-side dedup on item id (the §6.8 idempotent-apply rule).

## Open questions

- Item `meta`: do richer payloads ride in a `meta: Value` field on `Item`, or in a typed
  per-source extension record the item references? (Defer until a second source exists.)
- Outbox storage: a dedicated `outbox` table with a delivery cursor vs reusing the job queue
  (§6.10 ↔ jobs scope) — decide when the must-deliver path is built.
- Retention/compaction of channel history (the inbox grows forever today).
