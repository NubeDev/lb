# Multi-node + sync + SSE gateway slice (session)

- Date: 2026-06-26
- Scope: ../../scope/sync/sync-scope.md (+ bus, mcp, inbox-outbox, frontend)
- Stage: S3 — multi-node / sync / SSE (STAGES.md)
- Status: in-progress

## Goal

Build the S3 second-node + sync slice as a **vertical** slice through every layer (not "finish
the sync crate"). Two halves:

**PART 1 — make the node multi-node (headless):**
- Stand up a SECOND node in-process for tests (a hub + an edge), peers on the same Zenoh
  network. Edge vs hub is CONFIG/ROLE only — no `if cloud {…}` in `crates/`.
- Make the `mcp/dispatch` routing seam REAL: a tool call on node A resolves to an extension
  hosted on node B and routes over a Zenoh queryable, with callers and `authorize` unchanged.
  `caps::check` still runs on the calling node, workspace-first.
- Sync channel state edge↔hub per README §6.8 authority/merge: a message posted on the edge
  while offline applies idempotently on reconnect (the inbox is idempotent on `(channel,id)`).

**PART 2 — the SSE/HTTP gateway:**
- A browser reaches a REAL node over SSE/HTTP. The only UI file that changes is
  `ui/src/lib/ipc/invoke.ts` — swap the in-memory fake for a real transport; `channel.api`
  verbs and `ChannelView` stay identical.
- Push others' live messages + presence into the UI via SSE → `useChannel`'s `setItems` sink.

**Exit gate (S3), restated as the acceptance criterion:** a second node joins; a cross-node
tool call routes and is capability-checked; channel data syncs edge↔hub with idempotent offline
apply; the browser reaches a node over SSE/HTTP (replacing the S2 in-memory UI fake) and sees
live messages appear.

## What changed

### PART 1 — multi-node (headless)

- **bus** — a request/response primitive: `query.rs` (`declare_queryable` → `Responder`/
  `Incoming`, `query`), workspace-scoped via the same `ws_key` prefix as pub/sub. This is the
  transport the routed tool call rides on; a query on `ws/{id}/…` can never reach a queryable in
  another workspace (the wall on the request path).
- **host role** — `role.rs`: a `Role` enum (`Edge | Hub | Solo`) as **config**, plus
  `Node::boot_as(role)`. The only role-derived policy is `is_shared_authority()` (data authority,
  §6.8) — never a code branch in a capability/store/bus path. `Node::boot()` = `Solo`.
- **mcp routing seam made real** — the registry now holds a `Target` per extension: `Local`
  (a live instance) or `Remote { tools }` (routing entry). `dispatch.rs` calls the instance for
  `Local` and **routes over the bus queryable** (`route::call_key` = `mcp/{ext}/call`) for
  `Remote`. `serve.rs`/`serve_call` is the serving side: a node answers routed calls by running
  its *local* dispatch. Callers and `authorize` are unchanged; `caps::check` runs on the calling
  node, workspace-first, before any hop.
  - The registry moved to `RwLock<HashMap>` behind one `Arc<Registry>` so the local call path,
    the routed serve loop, and `reload` share one source of truth (instances are already
    `Arc<Mutex<…>>`, so a resolved `Target` dispatches to the very same instance). `load_extension`/
    `reload_extension` now take `&Node`.
- **host serve/remote** — `serve_ext` declares a wildcard `ws/*/mcp/{ext}/call` queryable and
  spawns the answer loop; `register_remote_extension` adds the calling-node routing entry.
- **host sync** (`sync.rs`, the `sync` crate's first real behavior, lifted into the host layer) —
  `sync_channel` subscribes to a channel's bus messages and **idempotently records** each into the
  local store (`§6.8` "Zenoh → idempotent apply"); `replay_history` re-publishes durable items so
  a node that was OFFLINE during the original posts catches up on reconnect. Append-style items +
  inbox upsert on `(channel,id)` make the merge conflict-free.

### PART 2 — the SSE/HTTP gateway + the UI swap

- **role/gateway** (was a stub) — an **axum** SSE/HTTP server fronting a real node. One route per
  file (FILE-LAYOUT §4): `POST /channels/{cid}/messages` (`post`), `GET …/messages` (`history`),
  `GET …/stream` (SSE: live `message` + `presence` events). Every route forwards to a
  capability-checked `lb_host` verb with the session principal — the gateway adds no authority.
  `state.rs` holds the node + principal; `Gateway::from_shared` lets two sessions front one node
  (the isolation test). The `node` binary mounts it when `LB_GATEWAY_ADDR` is set (config-driven
  role selection, the thin wiring layer §3.1 permits).
- **ui** — the promised one-file transport swap. `lib/ipc/invoke.ts` now picks: Tauri IPC (shell)
  → real **HTTP** to the gateway (`VITE_GATEWAY_URL` set, the browser build) → the in-memory fake
  (tests, unchanged). New `lib/ipc/http.ts` (the fetch transport) and `lib/channel/channel.stream.ts`
  (the SSE client). `useChannel` gained a subscription that folds OTHERS' live messages into its
  existing `setItems` sink — idempotent merge by id, the node's contract. `ChannelView`,
  `channel.api`, and the verb names are unchanged.

## Decisions & alternatives

- **Routing key is ext-specific (`mcp/{ext}/call`), not one per-workspace dispatch key** — so
  exactly the node hosting `{ext}` answers; no fan-out, no tie-break. The serving queryable
  wildcards the workspace (`ws/*/…`) so one declaration serves all workspaces, but a request only
  ever arrives on the key the *calling* node emitted (`ws/{principal.ws}/…`, authorized
  workspace-first) — so a node-B caller can never produce a `ws/A/…` request. Rejected: a single
  `mcp/dispatch` queryable per node (every node answers every call → ambiguous routing).
- **`mcp::call` gained `bus` + `ws` rather than injecting a routing closure** — the scope doc
  explicitly shaped `dispatch.rs` as "route via Zenoh queryable here," and mcp already had to know
  the workspace. A trait-object transport was heavier for no gain at this stage; revisit if a
  non-Zenoh transport ever appears.
- **The serving node does NOT re-authorize** — authorization is the *calling* node's job
  (`caps::check`, workspace-first), and the workspace wall on the queryable key means a routed
  request physically can't target another workspace. Re-checking on the serve side would need the
  principal on the wire (a bigger token-on-the-bus design) for zero added safety here. Recorded as
  an open question for when extensions on the hub touch *hub-authoritative* data.
- **Sync = idempotent apply + replay, not a transactional outbox yet** — README §6.8's mechanism
  is "change feeds → outbox → Zenoh → idempotent apply." For append-style channel items,
  persist-before-publish (already true) + idempotent apply + a replay verb already give
  at-least-once with conflict-free merge. The durable outbox with a delivery cursor (§6.10) is the
  next step (still open in inbox-outbox). Chose the minimal honest slice over building the outbox
  speculatively.
- **Sync lives in the host layer, calling the `sync` crate's intent** — the `sync` crate stays the
  §9 placeholder; the actual edge↔hub behavior is host wiring (it needs store+bus+inbox together).
  Avoided turning `sync` into a god-crate; it can absorb the reusable core when the outbox lands.
- **axum for the gateway** — the key-stack listed "Rust web framework TBD"; resolved to axum
  (tokio-native, first-class SSE, `tower::Service` so routes are testable with `oneshot`). Recorded
  in `Cargo.toml` and the key-stack follow-up.
- **Role is config on `Node`, not a `bool is_cloud`** — three named roles (§5) keep deployment
  intent explicit; `is_shared_authority()` is the single role-derived *data-authority* policy, the
  one axis §3.1 allows roles to differ on.

## Tests

Mandatory categories that apply at S3 and now exist **across two nodes**: **capability-deny**
(routed + gateway), **workspace-isolation** (routing seam, sync path, and gateway), and the FIRST
**offline/sync** tests (offline write → reconnect → idempotent apply + §6.8 merge). Determinism
held: item `ts` injected; **unique workspace id per test** (in-process peers share a workspace's
keyspace); **multi-thread flavor** on every node-booting test.

New this slice:
- **`host/cross_node_routing_test` (3)** — a call on the edge routes to hello on the hub and
  returns (S3 exit gate); routed call **denied** without the grant (refused on the edge, never
  routes); a ws-B principal **cannot route into ws-A** (isolation gate fires on the calling node).
- **`host/offline_sync_test` (3)** — offline edge writes apply **idempotently** on reconnect
  (history equals, ordered); duplicate replay does **not** duplicate (§6.8 merge); sync **never
  crosses the workspace wall** (ws-A replay never lands in the hub's ws-B).
- **`role/gateway/gateway_test` (4)** — post→history round-trips over **real HTTP**; the **SSE
  stream pushes a live message** posted by another session (the browser-feed story, over a real
  port); a post without the grant is **403** (mandatory deny); a ws-B session **can't read ws-A**
  through the gateway (mandatory isolation).
- **`ui/useChannel.test` (2, Vitest)** — a message arriving over the (mocked) SSE stream is folded
  into `items` via `setItems` (others' messages appear); the live merge is **idempotent** on id.

### Green output

Run per-binary / bounded parallelism — booting 2 nodes per test makes a single
`cargo test --workspace` OOM (debugging/bus/cargo-test-workspace-ooms-with-many-peers.md).

```
# Rust — light crates (real embedded SurrealDB / in-proc Zenoh where they touch it)
auth ........ 4    caps ....... 18    inbox ...... 4
bus ......... 2    ext-loader .. 2    store ...... 5     → 35 passed

# Rust — host integration (real wasm + real SurrealDB + 2 in-proc Zenoh nodes)
$ cargo test -p lb-host --test spine_test            → 4 passed   # S1 gate, still green post-refactor
$ cargo test -p lb-host --test messaging_test        → 3 passed
$ cargo test -p lb-host --test messaging_deny_test   → 3 passed   # MANDATORY deny
$ cargo test -p lb-host --test messaging_isolation_test → 2 passed # MANDATORY isolation
$ cargo test -p lb-host --test presence_test         → 2 passed
$ cargo test -p lb-host --test hot_reload_test       → 2 passed   # MANDATORY hot-reload, still green
$ cargo test -p lb-host --test cross_node_routing_test → 3 passed # NEW: routed call + deny + iso ACROSS NODES
$ cargo test -p lb-host --test offline_sync_test     → 3 passed   # NEW: offline→reconnect idempotent + §6.8 + sync iso
   host total: 22 passed

# Rust — the SSE/HTTP gateway (axum router via tower::oneshot + a real SSE socket)
$ cargo test -p lb-role-gateway                      → 4 passed   # roundtrip + LIVE SSE + 403 deny + ws iso

   RUST TOTAL: 61 passed, 0 failed   (was 51 host+light at S2; +3 routing +3 sync +4 gateway)

# Tauri shell command layer (headless — capability-checked path through the real node)
$ cd ui/src-tauri && cargo test                      → 2 passed   # still green post host-refactor

# UI (Vitest) + type-check + bundle
$ cd ui && pnpm test                                 → 8 passed (3 files)   # +2: useChannel live SSE merge
  ChannelView.test.tsx ..... 3   channel.api.test.ts ..... 3   useChannel.test.ts ..... 2
$ pnpm build                                         → tsc --noEmit clean; vite build ✓

# Formatting + file size
$ cargo fmt --all --check                            → FMT OK
$ bash rust/scripts/check-file-size.sh               → all 104 source files within 400 lines
```

## Debugging

One non-trivial breakage this session, with a debug entry:

- [bus/cargo-test-workspace-ooms-with-many-peers](../../debugging/bus/cargo-test-workspace-ooms-with-many-peers.md)
  — `cargo test --workspace` is OOM-killed (137) once every S3 test boots **two** nodes (= two
  Zenoh peers) and cargo/libtest run them all in parallel. Not a leak (single binaries are green);
  a runner-resource ceiling. Fixed by a documented run recipe (per-binary / `--test-threads=1`),
  not a code change. (Also hit: the separately-built wasm guests had been cleaned from their target
  dir; rebuilt with `cargo build --target wasm32-wasip2 --release` — the spine/hot_reload/cross_node
  tests panic loudly when the component is missing, by design, so it surfaced immediately.)

## Public / scope updates

- Promoted to `public/`: `sync` (new), `mcp` (routing now real), `frontend` (real transport +
  SSE), `bus` (queryable); refreshed `public/SCOPE.md` with the S3 row.
- Wrote the `scope/sync/sync-scope.md` (was a one-line TODO) and refreshed open questions in
  `mcp` (cross-node routing now shipped; serve-side re-auth + multi-host tie-break open), `bus`
  (queryable shipped; router-endpoint config still S7), `frontend` (the SSE swap landed — the
  one-file-change promise held), `inbox-outbox` (sync's idempotent apply shipped; the durable
  outbox with a delivery cursor still open).

## Follow-ups

- **Transactional outbox** with a delivery cursor (§6.10) — the durable must-deliver path; sync
  here is the append-style idempotent-apply subset. Next when must-deliver messages exist.
- **Serve-side authorization** when hub-hosted extensions touch hub-authoritative data — needs the
  principal/grant on the wire (a token-on-the-bus design). Open question in `mcp` scope.
- **Multi-host tie-break** when two nodes host the same extension (mcp open question).
- **Real session** (login → token → principal) replacing the demo principal in the gateway +
  Tauri `state.rs`; per-workspace gateway routing (the gateway URL currently fixes the workspace
  to the session — the UI `_ws` arg is plumbed but unused on the stream).
- **Router-endpoint config** (explicit edge→hub Zenoh endpoints) for a real cross-host deployment
  (S7); in-process peers auto-discover, which is enough to prove S3.
- A `#[lb_test]` harness baking in the multi-thread flavor + bounded node-boot concurrency.
- STATUS.md updated? **Yes** — Sync/SSE slice marked `shipped`; S3 exit gate met.
