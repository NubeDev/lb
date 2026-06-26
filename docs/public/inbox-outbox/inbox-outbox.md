# Inbox / outbox (as built)

One **normalized item** shape that every source collapses into, persisted as **state** in
SurrealDB behind the workspace wall. The bus moves a live copy (motion); the inbox keeps the
durable record (§3.3). Promoted from `scope/inbox-outbox/` after S2. The transactional **outbox**
(must-deliver) is not built yet — it needs a second node to deliver to.

## The item

`lb_inbox::Item` — `{ id, channel, author, body, ts }`.

- `id` is caller-supplied and **stable**: re-delivering the same `(channel, id)` upserts one row
  (idempotent delivery). The store record id is `inbox:<channel>__<id>`.
- `ts` is a caller-injected **logical** timestamp (no wall-clock — deterministic), the order key.

## Verbs (shipped S2)

- `record(store, ws, item)` — persist into the workspace namespace (idempotent on `(channel,id)`).
- `list(store, ws, channel)` — every item in a channel, **sorted by `ts` oldest→newest**. The
  generic store `list` is a pure filter; the inbox owns the order key and sorts in Rust.

## Guarantees

- **Workspace-isolated:** a `list` in workspace B never returns workspace A's items (items live in
  A's namespace). Tested across store + inbox.
- **Idempotent:** stable item ids make re-delivery safe — the precondition the future must-deliver
  outbox relies on (a receiver dedups on item id).
- **Capability-first:** `record`/`list` are raw verbs; the host `channel` service runs
  `caps::check` before them — no unauthorized path to an item.

## Not yet built

The transactional **outbox** (at-least-once must-deliver + receiver dedup), item `meta` for richer
payloads, unread/triage views, retention/compaction. See `scope/inbox-outbox/` open questions.
