# Bus scope

Status: scope. Defines the Zenoh topology, workspace key expressions, and message classes
(README §6.2). S1 ships the peer lifecycle + key scoping; pub/sub verbs land with the
messaging slice (S2).

> Read with: `../../README.md` §6.2 (event bus), §3.3 (state vs motion), §6.10 (outbox is
> the durability backstop, not raw pub/sub).

---

## Goal

An embedded Zenoh peer per node — the host process *is* a bus peer, no broker. Motion only
(§3.3). Every key is workspace-scoped (`ws/{id}/**`), making the workspace wall structural on
the bus just as namespace does in the store.

## What shipped in S1

- `bus::Bus::peer()` opens a default in-process peer session (solo node).
- `bus::ws_key(ws, rel)` prepends the `ws/{id}/` prefix; callers never write it. The matching
  capability (`bus:chan/*:sub`) is written *without* the prefix — the host adds it on both the
  cap-check and the key, so they always agree (auth-caps scope).

## What shipped in S2 (pub/sub + presence)

- `bus::publish(bus, ws, rel, payload)` — `put` onto `ws/{id}/{rel}` (channels use
  `chan/{cid}/msg/{id}`). Fire-and-forget motion; durability is the store's job.
- `bus::subscribe(bus, ws, rel)` → `Subscription` over a `ws/{id}/{rel}` key expr (channels
  subscribe `chan/{cid}/msg/**`); `recv()` yields payload bytes.
- `bus::declare_presence` / `watch_presence` — **presence via Zenoh liveliness tokens** under
  `ws/{id}/presence/{member}`; the watch uses `history(true)` so a late watcher still sees who is
  here, and Zenoh auto-retracts a token when a peer drops (no stale "online" flag).
- The host `channel` service runs `caps::check` (`bus:chan/{cid}:{pub|sub}`) before any of these;
  `post` persists to the inbox (state) then publishes (motion).

## TESTING RULE (constraint): bus tests use a UNIQUE workspace id per test

In-process Zenoh peers **auto-discover each other and share a workspace's keyspace** — two
sessions on `ws/acme/...` see each other's traffic (debugging/bus/
in-process-peers-share-the-keyspace.md). So every bus test must use its own workspace id, or
concurrent tests in one binary cross-talk. This is also the correct semantic (the workspace is
the wall); the isolation test encodes the converse (distinct ids never cross).

## Non-goals (S1)

- Pub/sub, queryables, liveliness verbs — landed with messaging (S2, above). S1 proved the peer
  boots in-process and the key scoping is correct.
- Router mode + upstream endpoints (edge↔hub) — config, lands S3.
- Message classification + the must-deliver outbox path — formalized when a second node exists.

## DECISION (constraint): Zenoh requires a multi-thread Tokio runtime

Zenoh's runtime panics under Tokio's **current-thread** scheduler. Consequences, recorded so
they aren't rediscovered (see debugging/bus/zenoh-needs-multi-thread-runtime.md):

- The `node` binary uses `#[tokio::main]` (multi-thread by default) — fine.
- Any **test** that boots a `Node` (and thus a bus peer) must use
  `#[tokio::test(flavor = "multi_thread", worker_threads = 1)]`, not the default `#[tokio::test]`.
- Tests that don't touch the bus (caps, auth, store) are unaffected.

This is a property of the dependency, not our design; one worker thread suffices.

## How it fits the core

- **State vs motion (§3.3):** the bus moves messages; state is the store's job. No persistence
  here — durable delivery is the outbox (§6.10).
- **Workspace wall (§7):** `ws_key` makes every key workspace-prefixed; a peer for ws A cannot
  name ws B's keys without the prefix the host controls.
- **Symmetric nodes (§3.1):** peer vs router is config, not code.

## Testing plan

- S1: `bus::ws_key` unit tests (prefix + leading-slash tolerance) — shipped, passing.
- S2 (shipped): pub→sub round-trip within a workspace (`host/messaging_test`); **mandatory
  isolation** — a sub in ws B never receives a publish in ws A (`host/messaging_isolation_test`);
  presence join/leave via liveliness + a presence deny test (`host/presence_test`).

## What shipped in S3 (request/response + the second node)

- `bus::declare_queryable` / `bus::query` — a workspace-scoped Zenoh **queryable**: the
  request/response transport the routed MCP tool call rides on (`mcp/{ext}/call`). The `ws/{id}/`
  prefix walls it exactly like pub/sub — a query in workspace B can't reach a queryable in A.
- The **second node** is config: `Node::boot_as(role)` opens the same peer; two in-process peers
  auto-discover into one network. Explicit router endpoints stay a deployment concern (S7).

## Open questions

- Message classification (fire-and-forget / must-deliver / must-replay, §6.2) — **still open**;
  the **append-style idempotent-apply** subset shipped via sync (`replay_history` + idempotent
  apply, see `../sync/sync-scope.md`); must-deliver still routes through the durable outbox.
- Explicit router endpoint config shape — deferred to S7 (in-process auto-discovery proved S3).
- Whether the live subscriber should also replay durable history on connect (today the UI reads
  `history` then subscribes; a combined "backlog + live" bus primitive could fold the two). Note
  the sync layer's `replay_history` is a node↔node variant of exactly this.
