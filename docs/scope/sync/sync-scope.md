# Sync scope

Status: scope. The edge↔hub authority partition (README §6.8) — NOT multi-master. The S3 slice
shipped the append-style idempotent-apply subset; the durable outbox with a delivery cursor
(§6.10) is the next step.

> Read with: `../../README.md` §6.8 (sync authority/merge), §3.3 (state vs motion),
> `../bus/bus-scope.md` (the bus moves items; the queryable carries routed calls),
> `../inbox-outbox/inbox-outbox-scope.md` (the inbox is idempotent on `(channel,id)` — the
> precondition this leans on), `../mcp/mcp-scope.md` (the routed tool call shares the same
> second-node substrate).

---

## Goal

Make edge and cloud **one binary differing only by config + data authority** (§3.1). Partition
data by authority so sync stays tractable without general multi-master:

- **Node-local data** — owned by one node, never synced.
- **Shared workspace data** — cloud(hub)-authoritative; edges hold a read-cache and queue writes
  up; merges cleanly because the writes are **append-style** and **idempotent**.
- **Real-time collab** (co-editing) — a per-extension CRDT concern, not a platform feature.

Mechanism (§6.8): SurrealDB change feeds → outbox → Zenoh → **idempotent apply**, last-writer-wins
on the rare contested shared record. One reusable crate; direction + authority are config.

## What shipped in S3 (the append-style subset)

Channel items are the first shared-data type, and they are the easy case: each is addressed by a
stable `(channel, id)` and the inbox `record` **upserts** on it. So sync is just idempotent apply
of items off the bus — no conflict resolution needed (immutable, distinct ids never collide).

- **Node role is config** — `lb_host::Role` (`Edge | Hub | Solo`) + `Node::boot_as(role)`. The only
  role-derived policy is `is_shared_authority()` (the hub/solo own shared data); no `if cloud` in a
  core path. The "second node" is just a second `Node::boot_as` — two in-process Zenoh peers
  auto-discover into one network.
- **`sync_channel`** (host) — subscribe to a channel's bus messages and idempotently `record` each
  into the local store. A live post on any peer lands in this node's durable history.
- **`replay_history`** (host) — re-publish this node's durable items onto the bus, so a node that
  was OFFLINE during the original posts catches up on reconnect. Idempotent apply makes replay
  always safe (including re-replay).
- The routed **MCP tool call** (mcp scope) rides the same second-node substrate: a call on the edge
  routes over a Zenoh **queryable** to the hosting hub, `caps::check` on the calling node first.

## How it fits the core

- **State vs motion (§3.3):** sync moves item *copies* over the bus (motion) and applies them to the
  store (state). Persist-before-publish (already true in `post`) means a missed live push is always
  recoverable — and replay turns that recovery into the offline-catch-up path.
- **Workspace wall (§7):** every bus key is `ws/{id}/…`, so a hub syncing workspace B can never
  apply workspace A's replayed items — proven by `sync_never_crosses_the_workspace_wall`.
- **Symmetric nodes (§3.1):** sync direction is *which node runs `sync_channel`/`replay_history`*,
  config — not a code branch. Edge and hub run identical code.
- **Idempotency (§6.8):** the stable `(channel,id)` upsert is the whole reason the merge is
  conflict-free; it is also the precondition the durable outbox will rely on (receiver dedups on id).

## Testing plan (mandatory categories apply)

- S3 (shipped): `host/offline_sync_test` — offline write → reconnect → **idempotent apply** + order
  (§6.8); duplicate replay does not duplicate; **workspace-isolation across nodes via the sync
  path**. `host/cross_node_routing_test` — the routed tool call's deny + isolation across two nodes.
- Outbox (next): a must-deliver message written transactionally, relayed at-least-once, deduped on
  the receiver — the §6.10 durability backstop.

## Open questions

- **Durable outbox + delivery cursor (§6.10):** a dedicated `outbox` table with a cursor vs reusing
  the job queue — decide when the first must-deliver (non-append-style) message exists. Today's sync
  is the append-style subset; mutable shared records need last-writer-wins on a contested row.
- **Change-feed driver:** S3 replays from the durable `list`; the §6.8 mechanism names SurrealDB
  *change feeds* as the trigger. Wire the change feed → outbox relay so sync is push-driven, not a
  manual `replay_history` call, once the outbox lands.
- **Serve-side authorization on routed calls:** a hub extension touching *hub-authoritative* data
  may need the principal/grant on the wire (token-on-the-bus). Today authorization is the calling
  node's and the workspace wall on the queryable key is the guarantee — sufficient while routed
  tools don't read hub-owned data. (Cross-ref mcp scope.)
- **Conflict policy surface:** last-writer-wins is the §6.8 default; do any shared types need a
  per-type merge (beyond append/LWW) before CRDT-in-extension territory?
- **Reusable crate boundary:** the S3 behavior lives in the host layer (it needs store+bus+inbox
  together); when the outbox lands, factor the reusable core into the `sync` crate (today a §9
  placeholder) without making it a god-crate.
