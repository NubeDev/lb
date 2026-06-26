# Bus (as built)

The event bus is an **embedded Zenoh peer** per node — the host process *is* a bus peer, no
broker. The bus moves **motion** only; durable **state** is the store's job (§3.3). Promoted from
`scope/bus/` after the S2 messaging slice.

## Topology

- One in-process peer per node (`Bus::peer()`). Edge/cloud peer-vs-router is config, not code
  (symmetric nodes, §3.1). A **second node** (S3) is just a second `Node::boot_as(role)` — two
  in-process peers auto-discover into one network; explicit router endpoints are a deployment
  concern (S7).
- **Every key is workspace-scoped:** callers pass a workspace-relative key; `ws_key(ws, rel)`
  prepends `ws/{id}/`. Callers never write the prefix — the host does, on both the cap-check and
  the key, so they always agree. This makes the workspace wall **structural** on the bus (§7): a
  peer for workspace B cannot *name* workspace A's keys.

## Verbs (shipped S2)

| Verb | Key | Purpose |
|---|---|---|
| `publish(bus, ws, rel, payload)` | `ws/{id}/{rel}` | fire-and-forget motion (channels: `chan/{cid}/msg/{id}`) |
| `subscribe(bus, ws, rel)` → `Subscription` | `ws/{id}/{rel}` (may use `*`/`**`) | live messages (channels: `chan/{cid}/msg/**`) |
| `declare_presence(bus, ws, member)` → `Presence` | `ws/{id}/presence/{member}` | a held **liveliness** token = "present" |
| `watch_presence(bus, ws)` → `PresenceWatch` | `ws/{id}/presence/*` | `(member, present)` changes; `history(true)` shows current |

### Request/response (shipped S3)

| Verb | Key | Purpose |
|---|---|---|
| `declare_queryable(bus, ws, rel)` → `Responder` | `ws/{id}/{rel}` | answer requests (each `Incoming` → `reply`) |
| `query(bus, ws, rel, request)` → `Option<reply>` | `ws/{id}/{rel}` | send a request, await the first reply |

A **queryable** answers a request — the transport the routed MCP tool call rides on
(`mcp/{ext}/call`). Workspace-scoped exactly like pub/sub, so a request in workspace B can never
reach a queryable declared in workspace A (the wall on the request path). `None` from `query` means
no node answered (e.g. the hosting node is offline).

## Presence = liveliness, not a stored flag

Presence rides Zenoh **liveliness tokens**: while a member holds the token they are present, and
when the peer drops — cleanly or by crash — Zenoh retracts it automatically. That auto-retract is
why presence is motion-derived and never persisted: a stored "online" flag would go stale on a
crash; a token cannot.

## Channels (state vs motion, end to end)

A channel is a bus subject *and* a durable inbox bucket. The host `channel` service is the
capability chokepoint: `post` runs `caps::check` (`bus:chan/{cid}:pub`), then **persists the item
to the store (state), then publishes it (motion)** — in that order, so a subscriber that missed
the live push always recovers from durable `history`. `subscribe`/`history`/presence require
`bus:chan/{cid}:sub`.

## Constraints (for anyone writing bus code or tests)

- **Multi-thread runtime required:** Zenoh panics under Tokio's current-thread scheduler. The
  `node` binary uses `#[tokio::main]`; any test that boots a Node must use
  `#[tokio::test(flavor = "multi_thread", worker_threads = 1)]`.
- **Bus tests use a unique workspace id per test:** in-process Zenoh peers auto-discover and
  share a workspace's keyspace, so reusing one id makes concurrent tests cross-talk. (Both
  recorded in `docs/debugging/bus/`.)

## Not yet built

Explicit router endpoints for cross-host deployment (S7; in-process peers auto-discover, enough for
S3); message classification + the must-deliver outbox path (the append-style idempotent-apply subset
shipped at S3 — see `../sync/sync.md`). See `scope/bus/bus-scope.md` open questions.
