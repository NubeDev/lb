# Sync (shipped — S3, append-style subset)

Edge↔cloud sync by the README §6.8 **authority partition** (NOT multi-master). Scope:
`../../scope/sync/sync-scope.md`. Session: `../../sessions/sync/multi-node-sync-session.md`.

## Node role is config, not a code branch

`lb_host::Role` (`Edge | Hub | Solo`) + `Node::boot_as(role)`. The same binary, built from the
same crates, plays every role; the only role-derived policy is `Role::is_shared_authority()` (the
hub/solo own shared data, edges hold a read-cache + queue writes up). No `if cloud` in any
capability/store/bus path (§3.1). The "second node" is just a second `Node::boot_as` — two
in-process Zenoh peers auto-discover into one network.

## What syncs, and how it merges

Channel items are the first shared-data type and the easy case: each is addressed by a stable
`(channel, id)` and the inbox **upserts** on it. So sync is **idempotent apply** of items off the
bus — no conflict resolution (items are immutable; distinct ids never collide). This is §6.8's
"Zenoh → idempotent apply" for the append-style subset.

- **`sync_channel(bus, store, ws, cid)`** → `ChannelSync` — subscribes to a channel's bus messages
  and records each into the local store. `apply_next()` applies the next item (idempotent). A live
  post on any peer lands in this node's durable history.
- **`replay_history(bus, store, ws, cid)`** — re-publishes this node's durable items onto the bus,
  so a node that was **offline** during the original posts catches up on reconnect. Replay is always
  safe (re-replay changes nothing — idempotent merge).

The offline→reconnect flow: an edge posts (persist-before-publish, so state is durable); the hub
misses the live push while disconnected; on reconnect the hub `sync_channel`s and the edge
`replay_history`s → the hub applies every item idempotently, history matches, in order.

## The routed MCP tool call (same second-node substrate)

A tool call on the edge whose extension is hosted on the hub **routes over a Zenoh queryable**
(`mcp/{ext}/call`), with callers and `authorize` unchanged. `caps::check` runs on the **calling**
node, workspace-first — the hub never sees an unauthorized call, and the workspace-scoped queryable
key means a ws-B caller can never reach ws-A data. See `../mcp/mcp.md`.

## The browser path (SSE/HTTP gateway)

A browser reaches a real node through the **gateway role** (`lb-role-gateway`, axum): `POST`/`GET`
`/channels/{cid}/messages` and an SSE `GET /channels/{cid}/stream` pushing live `message` +
`presence` events. Every route forwards to a capability-checked `lb_host` verb — no new authority.
This replaced the S2 in-memory UI fake; only `ui/src/lib/ipc/invoke.ts` changed transport. See
`../frontend/frontend.md`.

## Guarantees proven (tests)

- **Idempotent apply on reconnect** + order; **duplicate replay never duplicates** (§6.8 merge).
- **Workspace isolation across nodes** on all three new surfaces: the routing seam, the sync path,
  and the gateway (a ws-B principal/session never reaches ws-A data).
- **Capability-deny across nodes**: a routed call / a gateway POST without the grant is refused
  (on the calling node / as a 403) before any effect.

## Deferred

The **transactional outbox** with a delivery cursor (§6.10) — the durable must-deliver path; today's
sync is the append-style idempotent-apply subset. Change-feed-driven relay (vs the manual
`replay_history`), last-writer-wins on mutable contested records, serve-side authorization for
hub-authoritative data, and explicit router endpoints (S7). Tracked in the sync + mcp + inbox-outbox
scopes.
