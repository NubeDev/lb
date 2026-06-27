# Node-roles scope — fleet presence (admin sees connected nodes)

Status: scope (the ask). Promotes to `public/node-roles/` once shipped.

An operator opens the admin UI and sees **every node currently connected to a workspace** —
each one's stable id, its **deployment persona** (`hub` / `appliance` / `workstation`,
and the `mobile` client), role, version, and live online/offline state — updating in real
time as nodes join and drop. Today the platform tracks *channel-member* presence but has **no
node-level identity or roster**: there is no `NodeId`, the `Role` is config-only and never
surfaced at runtime, and no API lists connected nodes. This scope adds that fleet-visibility
surface, reusing the existing Zenoh-liveliness presence pattern rather than inventing a new
mechanism.

> Read with: `README.md` §5 (roles + the **deployment personas** table), §6.2/§6.4 (Zenoh
> liveliness for presence), §6.13 (admin section, SSE), `node-roles-scope.md` (roles are
> config), `bus/bus-scope.md` (the shipped `declare_presence`/`watch_presence` verbs this
> builds on), `auth-caps/` (the admin grant that gates the roster).

## Goals

- Give every running node a **stable `NodeId`** and attach its **persona + role + version** to
  that identity, read from config at boot (ties to the `LB_ROLE` config slice).
- Each node **announces itself** onto the bus via a Zenoh liveliness token at
  `ws/{id}/nodes/{node_id}`, auto-retracted on clean shutdown *or* crash (no stale "online").
- Expose an **admin-gated `nodes.list` MCP tool** + a gateway route so the admin UI renders a
  live roster (and a watch/stream for join/leave deltas).
- Put the **persona terms in code** (`hub`, `appliance`, `workstation`, `mobile`) so the
  backend speaks the same vocabulary the docs and UI do.

## Non-goals

- **Device/sensor registry, provisioning, onboarding, OTA, device shadows** — explicitly out
  (see `ingest/ingest-scope.md`: "a device = a principal"). This is *operator visibility of
  nodes*, not IoT fleet management.
- **Remote control of nodes** (restart/drain/evict from the UI) — a later admin-action slice;
  this scope is read-only observability.
- **Cross-workspace / global fleet view** — the roster is **workspace-scoped** like all keys
  (§3 rule 6). A super-admin global view is a separate, deliberately-privileged surface.
- **The `LB_ROLE` + Zenoh router/connect + `LB_STORE_PATH` config slice itself** — its own
  prerequisite slice; this scope *consumes* a real `NodeId`/role from it and notes the seam.

## Intent / approach

Node presence is **the channel-presence pattern, one level up.** `crates/bus/src/presence.rs`
already declares a liveliness token at `ws/{id}/presence/{member}` and watches it with
`history(true)` so a late watcher sees the current set; the token auto-retracts on disconnect.
We reuse exactly that for nodes at key `ws/{id}/nodes/{node_id}`, with the token payload (or a
companion record) carrying `{persona, role, version, started_at}`.

The roster is **motion, not durable state** — liveliness *is* the source of truth for "who is
online," so a node disappearing from the bus is the disconnect signal, with no record to
garbage-collect (the §3 rule-3 state-vs-motion line: don't persist a `connected` flag that can
go stale). An optional **last-seen** record in SurrealDB is the only durable part, written for
"seen 3m ago" history — and it's a cache, not authority.

**Why not a durable node registry** (a `nodes` table that rows insert/delete on connect): it
reintroduces exactly the stale-online bug liveliness was chosen to avoid (a crashed node leaves
a phantom row), and it duplicates what Zenoh already tracks. Rejected. The liveliness token is
authoritative for *online*; SurrealDB only remembers *last seen*.

**Persona in code.** Add a `Persona` value (`hub` / `appliance` / `workstation` / `mobile`)
distinct from `Role` (`Edge`/`Hub`/`Solo`): a persona is a *named deployment* of a role (README
§5), so `appliance` and `workstation` both map to `Role::Edge` and differ only by whether
the UI mounts. `mobile` is a **client**, not a node — it does not declare node presence; it
shows up (if at all) as a gateway *session*, a separate, lighter surface noted under open
questions. Keeping `Persona` separate from `Role` honours §3 rule 1: core crates still branch on
neither; the persona is descriptive metadata the roster displays.

## How it fits the core

- **Tenancy / isolation:** the roster key is `ws/{id}/nodes/...`; a node announces **per
  workspace it serves**. A ws-B admin never sees ws-A's nodes — same workspace-first wall as
  channel presence, and tested the same way.
- **Capabilities:** `nodes.list` requires an **admin-scoped grant** (e.g.
  `mcp:admin.nodes:list`), not a member default — fleet visibility is privileged (§6.13 admin
  is role-gated). The deny path: a `member` token calling `nodes.list` is refused by the cap
  check before any bus read.
- **Symmetric nodes:** no `if cloud {…}`. Every role announces presence with the **same** code;
  the *persona* is just the value it announces. A hub's roster-watch and an edge's are the same
  verb; only the admin UI typically subscribes.
- **One datastore:** the only durable piece is an optional `last_seen` record in SurrealDB. No
  new store. Online-state is liveliness (motion), not a table.
- **State vs motion:** online/offline is **Zenoh liveliness** (motion); last-seen is SurrealDB
  (state). We deliberately do **not** store the live `connected` flag.
- **MCP is the contract:** the roster is reached as MCP tools (`nodes.list`, `nodes.watch`),
  the same contract the UI, agents, and extensions use — no bespoke admin endpoint that
  bypasses the capability gate.
- **Durability:** N/A — presence is intentionally ephemeral; there is no must-deliver effect, so
  nothing goes through the outbox. A join/leave missed while the admin UI was closed is
  re-derived from `history(true)` on reconnect.
- **SDK/WIT impact:** none expected — node presence is declared by the **host** at boot, not by
  a wasm extension, so it does not touch the stable plugin boundary. (Flag if a future
  extension-health surface wants to publish into the same keyspace.)

## Example flow

1. `workstation` node boots. The config slice gives it `NodeId = node:7f3a…`, `Persona =
   workstation`, `Role = Edge`, and the workspaces it serves (e.g. `acme`).
2. On boot it calls `declare_node_presence(bus, "acme", node_id, {persona, role, version})` →
   a liveliness token at `ws/acme/nodes/node:7f3a…`. The token is **held** for the node's life.
3. An operator opens the admin **Fleet** panel. The UI calls `nodes.list` (admin grant present)
   → the host watches `ws/acme/nodes/*` with `history(true)` and returns the current set:
   the hub, this `workstation`, and an `appliance` Pi — each with persona + version + "online".
4. A second `appliance` joins → its token appears → `nodes.watch` pushes a join delta over
   SSE → the panel adds a row live.
5. The Pi loses power. Zenoh retracts its token within the liveliness timeout → a leave delta →
   the panel flips it to "offline", and (if enabled) a `last_seen` record stamps the drop time.
6. A `member`-role user calls `nodes.list` → **denied** at the capability gate; nothing leaks.

## Testing plan

Mandatory categories from `scope/testing/testing-scope.md` that apply:

- **Capability deny-test (mandatory):** a non-admin principal calling `nodes.list`/`nodes.watch`
  is refused; an admin grant succeeds. (Mirrors the existing channel-presence deny test in
  `host/presence_test`.)
- **Workspace-isolation (mandatory):** a node announcing in ws-A is **not** visible to a ws-B
  roster watch; two nodes in different workspaces never cross.
- **Offline / liveliness:** a node that drops (token retracted, incl. a simulated crash —
  drop without clean shutdown) disappears from the roster; a late watcher still sees the live
  set via `history(true)`.
- **Multi-node E2E:** extend the S3 `boot_as(role)` multi-node test — boot a hub + two edges,
  assert the roster lists all three with the right personas, then drop one and assert the leave.
- **Unit:** `Persona` ↔ `Role` mapping (`appliance`/`workstation` → `Edge`; `hub` → `Hub`);
  `NodeId` stability across a reconnect (same id re-announces, not a duplicate row).

## Risks & hard problems

- **Liveliness timeout vs. "feels live."** Zenoh retracts on a timeout, so "offline" can lag a
  hard power-cut by seconds. The UI must show "last seen", not imply millisecond accuracy.
- **A node serving many workspaces** announces N tokens (one per ws) — correct for isolation,
  but the count multiplies; confirm token cost is acceptable for a hub serving many workspaces.
- **`NodeId` stability** must come from durable config/keypair, not a per-boot random — else a
  restart looks like a new node and the roster grows ghosts. Depends on the config slice.
- **The `mobile` client** has no node identity; representing "3 phones connected" means counting
  **gateway sessions**, a different lifecycle (HTTP/SSE connection, not a Zenoh peer). Easy to
  conflate with nodes in the UI — keep them visually distinct.
- **Reconnect storms.** A flapping edge re-announces repeatedly; the watch must debounce deltas
  so the panel doesn't thrash.

## Open questions

- **Mobile/gateway sessions in the roster?** Show `mobile` clients as a separate "sessions"
  list derived from active gateway SSE connections, or omit from v1 and show nodes only?
  (Recommend: nodes-only v1; sessions a fast-follow.)
- **Token payload vs. companion record.** Put `{persona,role,version}` directly in the
  liveliness token value, or keep the token bare and carry metadata in a short-lived
  `ws/{id}/nodes/{id}/meta` record? (Token-value is simpler if Zenoh value size is fine.)
- **`last_seen` — in or out of v1?** Pure-liveliness (no durable row) ships smaller; last-seen
  history can follow. Default: ship without it, add if operators ask.
- **Admin grant name** — reuse an existing admin cap or mint `mcp:admin.nodes:list`? Settle with
  `auth-caps`.
- **Extension health** (§6.4 pairs liveliness with "extension health") — same keyspace
  (`ws/{id}/nodes/{id}/ext/*`) or its own scope? Likely its own; note the seam.

## Related

- `README.md` §5 (roles + **deployment personas** table — the source of the persona names),
  §6.2/§6.4 (liveliness presence), §6.13 (admin section).
- `node-roles-scope.md` (roles are config), `node-roles/deployment-personas-session.md` (where the
  persona names were decided), `bus/bus-scope.md` (shipped `declare_presence`/`watch_presence`).
- Code this builds on: `crates/bus/src/presence.rs`, `crates/host/src/channel/presence.rs`,
  `crates/host/src/role.rs` (`Role`), `role/gateway/src/routes/stream.rs` (SSE).
- Prerequisite: the `LB_ROLE` + Zenoh router/connect + `LB_STORE_PATH` config slice (gives a real
  `NodeId`/`Persona`/`Role` at boot) — see `node-roles/deployment-personas-session.md`.
