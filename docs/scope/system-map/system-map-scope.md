# System-map scope — a workspace topology + status console

Status: scope (the ask). Promotes to `public/system-map/` once shipped.

We want a **first-class, framework-level console** that lets a developer or operator *see the whole
system for one workspace at a glance* — which subsystems exist, whether each is healthy, and how they
connect. Two surfaces over one read: a **status grid** (a card per subsystem with its live numbers)
for "is it healthy", and a **react-flow topology** (nodes = subsystems, edges = who reaches whom) for
"what is connected". It is the debugging map you open first when something in the chain
(gateway → MCP → store / bus / outbox / job / extension) misbehaves and you need orientation before
you dive into logs.

> Read with: README §6.5 (MCP dispatch — how the read verb is reached), §6.13 (frontend — the shell
> this page lives in), §6.17 (observability — the **emitted** telemetry this view is the *read* complement
> of), §3.3 (state vs motion — the grouping the cards use). Siblings: `../observability/` (telemetry
> emission), `../frontend/ui-standards-scope.md` (the look this page must obey), the `dbview`
> host service (`store.*`, the precedent admin read-lens this mirrors), and the `fleet-monitor`
> extension (the per-node widget pattern this is one altitude above).

This is the **read/visualization** half. It does not emit telemetry (that's `observability/`) and it
does not own a durable record (the snapshot is derived live). It answers a question the platform could
*enforce* but never *show*: for this workspace, what is wired together and is it up?

## Why core, not an extension

An extension is the wrong home for a system map: it has to observe the **host that supervises
extensions**, including the extension runtime itself. An extension cannot truthfully report
"the extension service is down" — it would be reporting on the thing that hosts it (a chicken-and-egg
on liveness). The observer must sit in the host, beside `dbview`/`dashboard`, reading the booted
`Node`'s subsystem handles directly. So: a new host service (`host/src/system/`) exposing read verbs,
plus a first-class page in the shared UI shell (`ui/`), **not** a federated bundle.

## Goals

- **One snapshot, two views.** A single workspace-scoped read produces both the status grid and the
  topology graph — they must never disagree, so they project from one gathered result.
- **Every subsystem represented.** Gateway, Zenoh bus, MCP service, datastore, ingest, inbox, outbox,
  jobs, extension service, registry — a fixed set always present (so a missing card means "we forgot
  it", never "it happens to be empty"), each with live numbers.
- **Health that an operator can act on.** `Ok` / `Idle` (up, nothing flowing) / `Degraded` (up, wants
  attention — e.g. dead-lettered effects, an enabled-but-stopped extension) / `Down`. An empty queue
  is `Idle`, never a fault.
- **Workspace-walled and capability-gated** like every other read lens — admin-only, opaque deny.
- **Obeys the UI standard.** shadcn-first, `AppPageHeader`, responsive to a phone
  (`../frontend/ui-standards-scope.md`).

## Non-goals

- **No telemetry emission.** Logs/traces/metrics are `observability/` (§6.17). This view *reads*
  derived liveness; it does not add spans or a metrics pipeline.
- **No durable state.** The snapshot is a pure function of live subsystem state + the store at call
  time. No new table, no history/retention (that would be the observability/audit ledgers' job).
- **No control actions (v1).** Read-only. No restart/enable/kill from this surface — those verbs
  already exist (`ext.enable`/`disable`, lifecycle scope) and stay there. This is a *map*, not a
  control panel. (Wiring those actions in is a named open question, not this slice.)
- **No cross-workspace / fleet-wide roll-up.** One workspace at a time (the hard wall). A true
  multi-node fleet view is the `fleet-monitor` lineage / a later scope.
- **No new visual design.** Tokens, density, amber accent unchanged (`ui-design-scope.md`).

## Intent / approach

A new host service `host/src/system/` mirrors the `dbview` shape exactly (one verb per file, a single
gate, an opaque error), exposing two read verbs:

- `system.overview` → `SystemOverview { ws, role, services: ServiceStatus[] }` — the status grid.
- `system.topology` → `SystemTopology { ws, role, nodes, edges }` — the react-flow wiring.

Both authorize once (`mcp:system.overview|topology:call`, workspace-first) and then read **raw**
subsystem state through a shared `collect.rs` — *not* through the gated host wrappers (`ext_list`,
`outbox_status`), because those re-check *their own* caps; the snapshot is one capability, not the
union of every verb it summarizes (the same way `dbview` runs its admin gate once, then calls the raw
`lb_store::tables`). The gather leans on already-green, known-good reads:

- `lb_store::tables(store, ws)` — table list + exact row counts: the datastore card, and the derived
  counts for ingest / inbox / jobs / registry (matched by table name, degrading gracefully to `0`/`Idle`
  if a table doesn't exist yet — never an error).
- `lb_assets::list_installs` + `node.sidecars` — the extension card (installed / running / tier),
  `Degraded` when an extension is enabled but not running.
- `lb_outbox::{pending, delivered, dead_lettered}` — the outbox card, `Degraded` the instant anything
  is dead-lettered.
- `node.bus` / `node.registry` / `node.role` presence — the motion + runtime cards and the posture
  label (config, not a code branch — §3.1).

The **topology** projects the same `ServiceStatus[]` into nodes, and overlays the **architectural
wiring** as edges (gateway → mcp, mcp → store/bus, ingest → store, jobs → outbox, extensions → mcp,
registry → extensions, …), filtered to the nodes actually present so the graph never dangles. Edges
are the platform's fixed shape; node *health* is live.

**Transport:** a REST route pair (`GET /system/overview`, `GET /system/topology`) in the gateway,
exactly like `/store/*` — the admin read-lens precedent — with the cap re-checked server-side and the
workspace taken from the token, never the request. The host verb is *also* exposed through the one MCP
contract (`call_system_tool`, README §6.5/§7) so an agent can read the same snapshot it shows a human.

**Rejected alternative:** building this inside the `fleet-monitor` extension (or any extension) over
the series bridge. Rejected because (a) an extension can't observe the runtime that supervises it
(liveness chicken-and-egg, above), and (b) the series-read bridge only sees time-series, not the live
`Node` handles (sidecar map, registry, role) a true system map needs. The observer belongs in the host.

## How it fits the core

- **Tenancy / isolation:** every read is namespace-bound to `ws` (`lb_store` `use_ws`, the raw outbox/
  install reads select the ws namespace first). A snapshot physically cannot include another
  workspace's subsystems. **Isolation test is mandatory** (two workspaces, B's overview never shows A's
  rows/effects/extensions).
- **Capabilities:** gated `mcp:system.overview:call` / `mcp:system.topology:call`, **admin-only** by
  grant convention (a system snapshot reads across the whole workspace, like the `store.*` lens — so
  the cap rides the workspace-admin role, not the member set). Deny is opaque (`Denied` → `403`, no
  existence/detail signal). **Deny test is mandatory** (a token without the cap is refused).
- **Symmetric nodes:** no `if cloud`. The node `role` is surfaced as a *label* only; core paths never
  branch on it (§3.1). The same binary serves the same verbs in every posture.
- **One datastore:** no new persistence. The only reads are `lb_store::tables` + existing record reads;
  no new table, no second store.
- **No mocks / no fake backend (CLAUDE §9):** the gateway component test renders `SystemView` against a
  **real seeded node** through the existing `*.gateway.test.tsx` harness; the Rust test boots a real
  `Node`, seeds real records, and asserts the snapshot. No `*.fake.ts`. (Testing-scope §0.)
- **State vs motion:** the snapshot reads **state** (store + live handles); it is neither store-state
  it persists nor must-deliver motion. The card `group` field (`motion`/`state`/`workflow`/`runtime`)
  surfaces this split to the operator.
- **Stateless:** the service holds nothing durable — kill + respawn re-derives the identical snapshot.
- **MCP is the contract:** the two verbs are MCP tools (`call_system_tool`), reachable by agents and
  the UI through the one dispatch path, even though the UI's default path is the convenience REST route.

### MCP surface (API shape — §6.1)

- **Get / list:** `system.overview` and `system.topology` — two **read** verbs. Each is a
  whole-workspace snapshot (no id arg; the workspace is the scope). These are the only verbs.
- **CRUD:** **N/A** — read-only by design (Non-goals). The map mutates nothing; control verbs live in
  their own scopes (`ext.enable`/`disable`, lifecycle).
- **Live feed (SSE / watch):** **deferred** (open question). v1 is poll-on-demand + a manual Refresh,
  which is honest for a debugging console you open deliberately. A `system.watch` over the bus
  (liveliness for presence, periodic re-derive) is the natural follow-up; flagged below, not built now.
- **Batch:** **N/A** — single snapshot, no per-item fan-out.

No write verb has a must-deliver side effect, so **no outbox** involvement.

## Example flow

1. An admin opens the **System** page in the shell. `useSystem()` calls `system_overview` (→ `GET
   /system/overview`) with the session bearer token.
2. The gateway `authenticate`s the token, derives `(principal, ws)`, and calls
   `lb_host::system_overview(&node, &p, p.ws())`.
3. The host runs the `mcp:system.overview:call` gate (workspace-first), then `collect_services` reads
   `tables`, `list_installs` + sidecars, and the outbox lifecycle counts — all namespace-bound to `ws`.
4. It returns ten `ServiceStatus` cards. The outbox card is `Degraded` ("2 effect(s) dead-lettered");
   every other card is `Ok`/`Idle`. The grid renders, the degraded card sorts/highlights first.
5. The admin clicks **Topology**; `useSystem` calls `system_topology`, gets the same ten subsystems as
   nodes (colored by the same health) plus the wiring edges, and react-flow lays out the map. The
   dead-letter is now visible *in context* — the operator sees outbox → github-target is the failing hop.
6. A **member** (no admin cap) navigating to `/system/overview` gets a `403` — opaque; the nav entry is
   cap-gated away in the shell, and the gateway re-checks regardless.

## Testing plan

From `scope/testing/testing-scope.md`, the mandatory categories that apply:

- **Capability deny-test (mandatory):** a real token *without* `mcp:system.overview:call` is refused
  `403` at the gateway; the host verb returns `Denied`. Same for `system.topology`.
- **Workspace-isolation (mandatory):** seed records (an extension install, an outbox effect, rows) in
  workspace A; assert workspace B's `system.overview` shows none of them and B's counts are B's only.
- **Backend unit/integration (real `Node`):** boot a real node, seed real records, assert: the fixed
  service set is present; `tables`-derived counts match seeded rows; an enabled-but-stopped extension
  yields `Degraded`; a dead-lettered effect yields `Degraded`; an empty workspace yields all
  `Ok`/`Idle` (never `Down`/`Degraded`); topology nodes ⊇ overview ids and every edge endpoint is a
  present node (no dangling edge).
- **Gateway component test (real node, no fakes):** `SystemView.gateway.test.tsx` signs in to a real
  seeded workspace, renders the page, asserts the cards render with the seeded numbers and Refresh
  re-fetches. Extend the responsive smoke (render narrow, assert no horizontal overflow) per the UI
  standard.
- **Hot-reload / offline:** N/A to a read snapshot (nothing durable to survive), but the isolation +
  deny tests must pass on the persistent (`LB_STORE_PATH`) engine as well as `mem://`.

## Risks & hard problems

- **Table-name coupling.** Deriving ingest/inbox/jobs/registry counts by matching table names is
  graceful-but-fuzzy: a renamed table silently reads `0`. Mitigation: substring match + the fixed-card
  rule (a `0`/`Idle` card is visible and obviously wrong if a subsystem clearly has data), and a unit
  test that seeds each subsystem and asserts its card is non-zero. A stronger fix (each crate exposes a
  typed `status()`／a shared table-name registry) is a follow-up, noted in open questions.
- **Liveness is shallow.** "Gateway/bus/mcp = Ok" currently means "the handle exists", not a real
  health probe (no Zenoh peer count, no round-trip). That's honest for v1 (the process is up to answer
  the request at all) but must be *labeled* as such, not oversold as a deep health check.
- **Exact `count()` cost.** `lb_store::tables` takes an exact count per table — fine at admin/dev
  scale, not free on a million-row series table. The admin-only gate bounds who pays it; an estimate is
  a documented follow-up (same trade-off `dbview` already accepts).
- **Two views drifting.** The grid and graph must agree. Mitigation: both project from one
  `collect_services` result in a single call — never two independent gathers.
- **React-flow on mobile.** The topology graph is the hardest responsive piece; the status grid is the
  primary surface and must fully work on a phone, with the graph degrading to pan/zoom-in-a-card.

## Open questions

- **Live feed?** Add `system.watch` (bus liveliness + periodic re-derive over SSE, §6.13) so the map
  updates without a manual refresh — or is poll-on-open the right altitude for a debugging console?
- **Control actions inline?** Should `Degraded` cards offer the existing `ext.enable`/restart verbs in
  context (turning the map into a console), or stay strictly read-only? (Leaning read-only for v1.)
- **Typed subsystem status vs. name-matching.** Is it worth each crate exposing a small `status()` so
  the overview stops inferring counts from table names? (Leaning: ship name-matching now, revisit if it
  bites.)
- **Deep liveness probes.** Worth a real Zenoh peer count / store ping, or is handle-presence enough
  until the `observability/` metrics land and this view can read *those* instead of re-deriving?
- **Relationship to `observability/`.** Once telemetry ships, should this page read the metrics
  pipeline (deny rate, tool-call latency) rather than derive its own snapshot — i.e. become the *read
  UI* for observability? Likely yes; keep the verbs stable so the data source can swap underneath.

## Implementation status (already started)

A first cut of the backend exists on `master` (do not branch): `rust/crates/host/src/system/`
(`model.rs`, `error.rs`, `authorize.rs`, `collect.rs`, `overview.rs` — `topology.rs`, `tool.rs`, and
`mod.rs` still to write/finish). The implementing session continues from there: finish the module,
wire `lib.rs` exports, add the gateway routes + registration, grant the two caps in
`role/gateway/src/session/credentials.rs` (admin), build the UI feature (`ui/src/features/system/` +
`ui/src/lib/system/` + `http.ts` cases + NavRail/App registration), then satisfy this testing plan and
write the session/debug/public docs per `ABOUT-DOCS.md`.

## Related

- README `§6.5` (MCP/dispatch — how the verb is reached), `§6.13` (frontend shell), `§6.17`
  (observability — the emit half this reads beside), `§3.1` (symmetric nodes / role-as-config),
  `§3.3` (state vs motion).
- `scope/observability/observability-scope.md` — telemetry **emission** (the sibling; this is the
  read/visualization complement).
- `scope/frontend/ui-standards-scope.md` — the shadcn-first / responsive standard this page obeys;
  `scope/frontend/ui-design-scope.md` — tokens/look.
- `scope/extensions/lifecycle-management-scope.md` — where the control verbs (`ext.enable`/`disable`)
  live, deliberately *not* duplicated here.
- Precedent code: `rust/crates/host/src/dbview/` (the admin read-lens shape this mirrors),
  `rust/crates/host/src/ext/list.rs` (`ext_list` — the install+sidecar join),
  `rust/extensions/fleet-monitor/` (the per-node widget pattern, one altitude below).
- `docs/FILE-LAYOUT.md` (one responsibility per file), `docs/scope/testing/testing-scope.md` (§0 no
  fakes; deny + isolation mandates).
