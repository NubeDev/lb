# System map

The framework-level, **read-only** workspace **topology + status console** — the map you open first
when something in the chain (gateway → MCP → store / bus / outbox / job / extension) misbehaves and you
need orientation before diving into logs. Two surfaces over **one** workspace-scoped read: a **status
grid** (a card per subsystem with its live numbers) for "is it healthy", and a **react-flow topology**
(nodes = subsystems, edges = who reaches whom) for "what is connected". Both project from one gathered
result, so they can never disagree.

This is the read/visualization half. It emits no telemetry (that is `observability/`) and owns no
durable record — the snapshot is a pure function of live subsystem state + the store at call time, so a
node restart re-derives the identical map.

## Why a host service, not an extension

An extension can't truthfully report "the extension service is down" — it would be reporting on the
thing that hosts it. The observer must sit in the **host**, beside `dbview`/`dashboard`, reading the
booted `Node`'s subsystem handles directly. So: a host service (`rust/crates/host/src/system/`) exposing
two read verbs, plus a **first-class page in the shared shell** (`ui/src/features/system/`) — not a
federated bundle.

## The two verbs

Both authorize **once** (workspace-first, then the verb's `mcp:system.*:call` cap), then read raw
subsystem state through a shared `collect.rs` — **not** through the gated host wrappers (`ext_list`,
`outbox_status`), which re-check their own caps. The snapshot is *one* capability, not the union of every
verb it summarizes (the same way `dbview` runs its admin gate once, then calls the raw `lb_store::tables`).

- **`system.overview`** → `SystemOverview { ws, role, services: ServiceStatus[] }` — the status grid.
- **`system.topology`** → `SystemTopology { ws, role, nodes, edges }` — the react-flow wiring.
- **`system.subsystem`** → `SubsystemDetail { ws, role, service: ServiceStatus, extra }` — the detail
  of **one** subsystem (the `id` arg), so a card with no owning page (`gateway`/`bus`/`mcp`) drills into
  a real view instead of dead-ending. The `service` is the **same** card the grid shows (one gather);
  `extra` is a subsystem-specific JSON blob — for `bus`, `{ peer_zids: [...], router_zids: [...] }` (the
  live identities behind its peer/router counts); `{}` for every other subsystem. Authorizes once via
  the **same** gate as the other two (cap `mcp:system.subsystem:call`, admin-only). An **unknown id** is
  opaque `Denied` (no "which ids exist" signal, never a panic) — the only verb that takes an id.

- **`system.tools`** → `SystemTools { ws, role, tools: ToolInfo[] }` — the **full catalog of MCP tools
  reachable for the workspace**, with descriptions (tool-catalog scope). Both halves of the surface:
  the built-in **host-native** verbs (`host.*`/`system.*`/`agent.*`/`bus.*`/`store.*`/`inbox.*`/
  `outbox.*`/…, from a static host catalog with hand-written one-liners) **and** every
  **extension-contributed** tool from the runtime registry (`<ext>.<tool>`, name-only — the registry
  carries no description). Each `ToolInfo { tool, description, source, group }`; `source` is `"host"` or
  the ext id. Cap `mcp:system.tools:call`, admin-only. The read behind the **MCP service page**.
- **`system.acp`** → `AcpInfo` — the **ACP (Agent Client Protocol) adapter's** static protocol/
  capability facts (protocol version, handled `session/*` methods, advertised capabilities, JSON-RPC
  error codes, auth notes) — mirrors `role/acp`'s handshake; the host owns the truth so the UI never
  imports the role binary. ACP is a per-stdio-session adapter, **not a polled server**, so this is
  *reachable capability info, not a live health feed*. Cap `mcp:system.acp:call`, admin-only. The read
  behind the **ACP service page**.

`overview`/`topology`/`tools`/`acp` are whole-workspace snapshots (no id arg — the workspace is the
scope); only `subsystem` takes a single subsystem id. **Read-only by design**: the map mutates nothing
(the tool catalog lists tools, it never calls them); control verbs (`ext.enable`/`disable`, lifecycle)
stay in their own scopes.

### Service pages — MCP & ACP (tool-catalog scope)

The `mcp` and `acp` runtime cards now own **dedicated shell pages** (`/system/mcp`, `/system/acp`),
drilled from the System grid:

- **MCP service page** (`ui/src/features/system-mcp/`) — the live runtime counts plus a **searchable,
  source-grouped table of every reachable tool** with its description. Extension tools with no stored
  description show "no description provided" (honest, not hidden).
- **ACP service page** (`ui/src/features/system-acp/`) — the adapter's facts as labelled sections,
  honest that it reports capabilities, not a live connection count.

An **`acp` subsystem card** (Idle — available, per-session, not a polled resident) now sits in the grid
alongside `mcp`, with an `acp → mcp` topology edge ("drives the agent"). Both pages are reached by
drilling from the System page (not in the sidebar), each cap-gated; the gateway re-checks server-side.

### Wire shapes

```
ServiceStatus { id, label, group, health, detail, metrics: [{label, value}] }
SystemOverview { ws, role, services: ServiceStatus[] }
TopoNode { id, label, group, health }   TopoEdge { from, to, label }
SystemTopology { ws, role, nodes: TopoNode[], edges: TopoEdge[] }
SubsystemDetail { ws, role, service: ServiceStatus, extra }
  // extra for `bus`: { peer_zids: string[], router_zids: string[] }; {} otherwise
ToolInfo { tool, description, source, group }   // source: "host" | <ext id>
SystemTools { ws, role, tools: ToolInfo[] }
AcpInfo { protocol_version, methods: string[], capabilities: Metric[], error_codes: Metric[], notes: string[] }
```

- **`id`** — the stable key topology edges reference: `gateway`, `bus`, `mcp`, `extensions`, `registry`,
  `store`, `ingest`, `inbox`, `outbox`, `jobs`, plus `acp` (the Agent Client Protocol adapter). This
  **fixed set is always present** — a missing card means "we forgot it", never "it happens to be empty".
- **`group`** — `motion` / `state` / `workflow` / `runtime`, mirroring the core's state-vs-motion split
  (§3.3) and how the grid/graph band the cards.
- **`health`** — `ok` / `idle` / `degraded` / `down`. **`idle` is up-but-nothing-flowing** (an empty
  queue is healthy, never a fault); **`degraded`** is up-but-wants-attention (a dead-lettered effect, an
  enabled-but-stopped extension).
- **`role`** — the node's configured posture (`edge`/`hub`/`solo`), surfaced as a **label only** —
  config, not a code branch (§3.1).

## How the snapshot is gathered

`collect.rs::collect_services` builds the fixed card set from already-green, known-good reads, every one
namespace-bound to `ws`:

- `lb_store::tables(store, ws)` — table list + exact row counts: the datastore card, and the ingest /
  inbox / jobs / registry counts (substring-matched by table name, degrading gracefully to `0`/`idle` if
  a table doesn't exist yet — never an error).
- `lb_assets::list_installs` + `node.sidecars` — the extension card (installed / running / tier);
  **`degraded`** when an extension is enabled but not running.
- `lb_outbox::{pending, delivered, dead_lettered}` — the outbox card; **`degraded`** the instant
  anything is dead-lettered.
- `lb_bus::bus_stats(node.bus)` — **real Zenoh transport liveness** from the live session: this node's
  `zid`, and the count of peers/routers it is actually connected to on the mesh (`session.info()
  .peers_zid()`/`routers_zid()`) — not handle-presence. The bus card is `idle` when nothing else is on
  the mesh (a solo node with 0 peers is honest, not a fault) and `ok` once connected to ≥1 peer/router.
- `node.registry.summary()` — the live count of reachable extensions and total tools they expose (the
  MCP/runtime card's real numbers).
- `node.role` — the posture label, surfaced on the gateway card.

`system.topology` re-uses the **same** `collect_services` result, projects it into nodes, and overlays a
**fixed** architectural `WIRING` table (gateway→mcp, mcp→store/bus/extensions, ingest→store, jobs→outbox,
registry→extensions, …), filtered to the nodes actually present so **no edge dangles**. Edges are the
platform's fixed shape; node *health* is live.

## Transport

A REST route pair in the gateway, exactly like `/store/*`:

- `GET /system/overview` → `SystemOverview`
- `GET /system/topology` → `SystemTopology`
- `GET /system/subsystem/{id}` → `SubsystemDetail` (the only route with a path arg; an unknown id is
  `403`-opaque, like a denial)

Each re-runs the host gate server-side; the **workspace + principal come from the token, never the
request** (the hard wall, §7). A denied caller gets an opaque `403`. The verbs are *also* reachable
through the one MCP contract (`call_system_tool`, §6.5/§7), so an agent reads the same snapshot it shows
a human. The UI's default path is the convenience REST route.

## Capabilities

Gated `mcp:system.overview:call` / `mcp:system.topology:call` / `mcp:system.subsystem:call`,
**admin-only by grant convention** — a
system snapshot reads across the whole workspace, so the caps ride the workspace-admin role (beside the
`store.*` lens), not the member set. Deny is opaque (`Denied → 403`, no existence/detail signal). The
shell hides the **System** nav entry for a session lacking the cap; the gateway re-checks regardless.

## The shell page

`ui/src/features/system/` — a first-class page obeying the UI standard (shadcn-first, `AppPageHeader`,
responsive):

- **Status grid** (primary, fully works on a phone): a `Card` per subsystem with its health dot, the
  one-line detail, and the live metrics; **degraded cards sort first**. Reflows 1 → 2 → 3 columns. A
  card whose subsystem has a first-class page (`store`/`ingest`→data·ingest, `inbox`/`outbox`,
  `extensions`/`registry`→Extensions) is **clickable** — it drills into that page (keyboard-operable,
  hover ring, an ↗ affordance), so the map is an entry point, not a dead end. The link is gated to
  surfaces the session is allowed; the gateway re-checks regardless. Mapping lives in
  `features/system/navigate.ts`. A card with **no owning page** (`gateway`/`bus`/`mcp`) is **also**
  clickable (a `+` affordance, `aria-label "subsystem <id>"`): it opens an in-place **detail sheet**
  (`SubsystemDetailSheet`, a shadcn side drawer) loading `system.subsystem` — health, group, role, every
  metric, and for the Zenoh `bus` the **live peer/router zid lists** (the identities behind the counts;
  honest "none connected (solo on the mesh)" when the mesh is empty). So **every** card now leads
  somewhere — a page if it has one, the detail sheet otherwise.
- **Topology** (Grid/Graph toggle, lazy-loaded `@xyflow/react`): each subsystem a node coloured by live
  health, banded by group, with the fixed wiring as edges; degrades to pan/zoom on a phone.
- **Refresh** re-fetches on demand — poll-on-open, honest for a debugging console you open deliberately
  (a live `system.watch` feed is a flagged follow-up, not v1).

## Guarantees (what the tests prove, against a real node — no mocks)

- The fixed service set is **always present**; an **empty workspace** yields every card `ok`/`idle`
  (never `down`/`degraded`).
- `tables`-derived counts **match seeded rows**; an enabled-but-stopped extension and a dead-lettered
  effect each yield **`degraded`**.
- Topology nodes ⊇ overview ids; **every edge endpoint is a present node** (no dangling edge).
- **Capability-deny** (a no-cap token is refused; one verb's cap does not grant the other) and
  **two-workspace isolation** (B's snapshot shows none of A's state) — both mandatory, both green.
- The shell page renders the live numbers and Refresh re-fetches against a **real spawned gateway**, with
  a narrow-viewport responsive smoke.

## Limits / follow-ups (honest v1 boundaries)

- **Liveness depth varies, honestly.** The **bus** card now reports *real* connectivity (live Zenoh
  peer/router counts + zid), and its detail view lists the **actual connected peer/router zids** (the
  identities, not just a count); **mcp** reports the live extension/tool surface — not handle-presence.
  `gateway`/`store` are still "the handle is up to answer at all" (no round-trip probe); that's labelled,
  not oversold. A pub→sub echo probe is a noted follow-up.
- **Table-name coupling.** Ingest/inbox/jobs/registry counts are inferred by table-name substring match;
  a renamed table silently reads `0`. A typed per-crate `status()` is the noted stronger fix.
- **No live feed yet.** Poll-on-open + manual Refresh; `system.watch` over the bus is the natural next
  step. No control actions inline (read-only by design). No cross-workspace/fleet roll-up (the hard wall).

## Source

- Host service: `rust/crates/host/src/system/` (`model`/`error`/`authorize`/`collect`/`overview`/
  `topology`/`subsystem`/`tool`/`mod`), exported from `rust/crates/host/src/lib.rs`.
- Gateway: `rust/role/gateway/src/routes/system.rs` (+ `server.rs`, `routes/mod.rs`,
  `session/credentials.rs`).
- Real stats: `rust/crates/bus/src/stats.rs` (`bus_stats`/`BusStats`),
  `rust/crates/mcp/src/registry.rs` (`Registry::summary`/`RegistrySummary`).
- UI: `ui/src/lib/system/`, `ui/src/features/system/` (incl. `navigate.ts` — the id→surface drill-in
  map; `SubsystemDetailSheet.tsx` + `useSubsystemDetail.ts` — the no-page detail drawer),
  `ui/src/components/ui/card.tsx` + `sheet.tsx`, and the `NavRail`/`App`/`admin-caps` registration.
- Tests: `rust/crates/host/tests/system_map_test.rs`,
  `ui/src/features/system/SystemView.gateway.test.tsx`.
- Scope + rationale: [`../../scope/system-map/system-map-scope.md`](../../scope/system-map/system-map-scope.md).
