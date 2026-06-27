# Session — `system-map`, the workspace topology + status console (both halves)

**Date:** 2026-06-27
**Area:** observability / frontend (host read-lens + first-class shell page)
**Status:** shipped — code + real tests + docs; backend + UI green.
**Scope:** [../../scope/system-map/system-map-scope.md](../../scope/system-map/system-map-scope.md)
**Public:** [../../public/system-map/system-map.md](../../public/system-map/system-map.md)

## Goal

Finish the `system-map` scope to fully built: a **first-class, framework-level console** that shows
the whole platform for one workspace at a glance — a status grid (a card per subsystem with its live
numbers + health) and a react-flow topology (nodes = subsystems, edges = who reaches whom), both
projected from **one** workspace-scoped read so they can never disagree.

The backend's first cut already existed on `master`
(`rust/crates/host/src/system/` — `model.rs`, `error.rs`, `authorize.rs`, `collect.rs`, `overview.rs`).
This session continued from there: wrote the missing host files, wired the gateway transport, granted
the caps, built the UI feature, and satisfied the testing plan.

## What shipped

**Backend** — host service `rust/crates/host/src/system/`, mirroring the `dbview` admin read-lens shape
exactly (one verb per file, a single gate, an opaque error, an MCP `tool.rs` dispatcher):

- `topology.rs` — `system_topology`: re-uses the **same** `collect_services` gather as the overview,
  projects it into graph nodes (1:1, minus metrics), and overlays the platform's **fixed** architectural
  wiring as edges (a `WIRING` const: gateway→mcp, mcp→store/bus/extensions, ingest→store, jobs→outbox,
  registry→extensions, …), filtered to present nodes so the graph **never dangles**.
- `tool.rs` — `call_system_tool`: the one MCP contract over both verbs (host-native, not in the runtime
  `Registry`); each authorizes first, denials stay opaque (`ToolError::Denied`).
- `mod.rs` — module wiring + `pub use`, and the existing exports re-surfaced.
- `lib.rs` — `mod system;` + the `system::{…}` re-export (the host's public surface).

Gateway transport, mirroring `/store/*`:

- `role/gateway/src/routes/system.rs` — `GET /system/overview` + `GET /system/topology`, each
  authenticates the token, takes `ws` from the token (never the request), re-runs the host gate
  server-side, and maps `SystemError::Denied → 403` (opaque).
- registered in `server.rs`, exported in `routes/mod.rs`.
- `session/credentials.rs` — granted `mcp:system.overview:call` + `mcp:system.topology:call` beside the
  `store.*` admin caps (admin-only by grant convention — a snapshot reads across the whole workspace).

**UI** — a **first-class shell page** (not a federated extension), obeying the UI standard
(shadcn-first, `AppPageHeader`, responsive):

- `ui/src/lib/system/` — `system.types.ts` (the wire shapes mirroring the host types) + `system.api.ts`
  (`systemOverview`/`systemTopology` over `invoke`); `http.ts` cases `system_overview`/`system_topology`
  (GET `/system/overview` · `/system/topology`, following the `store_graph` case).
- `ui/src/components/ui/card.tsx` — generated the missing shadcn `card` primitive, repointed at the
  Lazybones tokens (`bg-card`/`fg`/`border`/`muted`) the way `sidebar.tsx` binds the upstream component.
- `ui/src/features/system/` — `useSystem` (poll-on-open + manual Refresh; lazy topology load),
  `SystemView` (the status-card grid, `AppPageHeader`, Grid/Graph tabs, degraded-first sort),
  `SystemTopologyGraph` (lazy-loaded react-flow, nodes coloured by live health, banded by group),
  `health.ts` (the one health→token map both surfaces share), `index.ts`.
- registration: `NavRail.tsx` (`CoreSurface` + `SURFACES`, icon `Network`), `App.tsx`
  (`allowed.push("system")` gated on `CAP.systemOverview`, the render branch, the import),
  `admin-caps.ts` (`CAP.systemOverview`/`systemTopology`).

## Testing (no mocks — testing-scope §0)

**Rust** (`crates/host/tests/system_map_test.rs`, real booted `Node` + real seeded records): fixed
service set always present; `tables`-derived counts match seeds (inbox=2, jobs=1 from real `write`s);
an enabled-but-stopped native install ⇒ extensions `Degraded`; a dead-lettered effect ⇒ outbox
`Degraded`; an empty workspace ⇒ every card `Ok`/`Idle`; topology nodes ⊇ overview ids and **no edge
dangles**. Plus the **mandatory** capability-deny (a no-cap token is refused; holding `overview` does
not grant `topology`) and **two-workspace isolation** (B's snapshot shows none of A's rows/effects/
extensions). **5/5 green.**

**UI** (`features/system/SystemView.gateway.test.tsx`, real spawned gateway via `pnpm test:gateway`):
the fixed grid renders with live numbers (store rows = 2 committed samples); a seeded dead-lettered
effect surfaces the outbox card as `Degraded`; Refresh re-fetches (Idle → Degraded, no remount); the
Graph toggle mounts the react-flow topology; **plus a narrow-viewport (360px) responsive smoke** (no
horizontal overflow) per the UI standard. **5/5 green** (the full gateway suite stayed green at 70/70).
Nav cap-gating extended in `NavGating.test.ts` (a member without `system.overview` never sees the entry).

`pnpm lint` stays at **0 errors** (the new files are fully shadcn-clean — the Grid/Graph tabs use the
`Button` primitive, not a raw `<button>`); `npx tsc --noEmit` clean.

## Notes / decisions

- **One gather, two projections.** `system_topology` deliberately calls the same `collect_services` as
  `system_overview` rather than a parallel gather — the scope's "the two views must never disagree" is
  structural, not a discipline.
- **Edges are fixed shape, health is live.** The wiring is a `const` in `topology.rs` (the platform's
  architecture), filtered to present nodes; only node *health* comes from the live snapshot.
- **Admin-only, re-checked server-side.** The nav entry is cap-gated in the shell for convenience, but
  the gateway re-runs the gate regardless (the deny is proven in the Rust test, not just hidden in the UI).

## Session 2 — real stats + clickable cards (follow-on)

Two gaps the first cut left, raised by the user: "Zenoh is up" reported nothing real, and the cards were
dead ends.

**Real subsystem stats** (no more handle-presence theater):

- `lb-bus` gained `stats.rs::bus_stats(&Bus) -> BusStats { zid, peer_count, router_count }`, reading the
  live `zenoh::Session::info()` (`peers_zid()`/`routers_zid()` enumerate established transports —
  local session bookkeeping, no round-trip). `collect.rs` now builds the **bus** card from it: `idle`
  on a solo node with 0 peers (honest), `ok` once connected; metrics `peers` · `routers` · `node zid`.
- `lb-mcp` `Registry` gained `summary() -> RegistrySummary { extensions, tools }` (a read-locked walk of
  the routing map); the **mcp** card now shows the live extension + tool surface.
- the **gateway** card surfaces the node `role` as a metric.

**Clickable drill-in** (cards → existing pages, per the user's "navigate to existing pages" choice):

- `features/system/navigate.ts` — the one `subsystem id → CoreSurface` map (`store`/`ingest`→data·ingest,
  `inbox`/`outbox`, `extensions`/`registry`→Extensions). `gateway`/`bus`/`mcp` have no page → `null` →
  stay static (no broken link).
- `SystemView` takes `onNavigate?` + `allowedSurfaces?`; a card with a mapped, allowed surface renders as
  a keyboard-operable control (`role="button"`, Enter/Space, hover ring, ↗ affordance, `aria-label`
  `open <id>`) that switches the shell surface. `App.tsx` passes `setSurface` + `allowed`. Link is gated
  to allowed surfaces; the gateway re-checks regardless.

**Tests:** `SystemView.gateway.test.tsx` grew **5 → 7** — the bus card exposes real `peers`/`routers`/
`node zid` metrics; clicking the outbox card calls `onNavigate("outbox")` while the bus card has no
`open bus` control. Backend re-verified in a clean `HEAD` worktree: `cargo test -p lb-host --test
system_map_test` **5/5**, `cargo build -p lb-role-gateway` clean. Full `pnpm test:gateway` **72/72**;
`tsc`/`lint` (0 err) green. Touched crates `cargo fmt`'d.

## Build/test caveat (unrelated, concurrent session)

A **separate concurrent AI session** has an in-flight, non-compiling refactor of `rust/crates/runtime/`
(`Cargo.toml`, `bindings.rs`, `instance.rs`, `lib.rs`, new `bridge.rs` — a `HostBridge` trait mid-change
that currently fails `dyn` compatibility + a missing `async_trait` dep). Because `lb-host` depends on
`lb-runtime`, that breakage blocks a whole-workspace `cargo build`/`cargo test` **through no fault of the
system-map code**. Per the task's "stay scoped, don't touch unrelated work" constraint, that crate was
left untouched. The system-map backend was verified green by running its tests in a **clean git worktree
at `HEAD`** (intact runtime) — `cargo test -p lb-host --test system_map_test` → 5/5, and
`cargo build -p lb-role-gateway` clean. The earlier in-session `cargo build --workspace` (before the
other session broke runtime) was also green. The UI gateway suite passed against the real spawned gateway.

## Session 3 — subsystem detail view (no-page cards stop being dead ends)

**Goal.** The cards with **no owning page** (`gateway`/`bus`/`mcp`) were static dead ends. Make every
card clickable: a card with a page still navigates (Session 2 behavior); a card without one opens a real
**detail view** — the Zenoh bus getting the richest one (a live peer/router zid list).

**What changed (files).**

- **`rust/crates/bus/src/stats.rs`** — `BusStats` now also carries `peer_zids: Vec<String>` +
  `router_zids: Vec<String>` (the actual connected ZIDs from `peers_zid()`/`routers_zid()`, not just
  counts). Still one cheap local `session.info()` read, no round-trip. `peer_count`/`router_count` are
  now derived from the list lengths so they can't disagree with the lists.
- **`rust/crates/host/src/system/`** — a third read verb:
  - `model.rs` — new `SubsystemDetail { ws, role, service: ServiceStatus, extra: Value }`.
  - `collect.rs` — `collect_extra(node, id)` → the subsystem-specific blob (`bus` → `{peer_zids,
    router_zids}`; `{}` for everything else).
  - `subsystem.rs` (new, one verb/file) — `system_subsystem(node, p, ws, id)`: the EXISTING
    `authorize_system` gate (new cap `mcp:system.subsystem:call`), then reuse `collect_services`, pick
    the requested card, attach `collect_extra`. An unknown id → opaque `Denied` (no "which ids exist"
    signal, never a panic).
  - `tool.rs` — `call_system_tool` dispatches `system.subsystem` (reads `{"id":…}` from input).
  - `mod.rs` + `lib.rs` — export the verb + type.
- **`rust/role/gateway/`** — `GET /system/subsystem/{id}` in `routes/system.rs`, registered in
  `server.rs`, exported in `routes/mod.rs`; cap `mcp:system.subsystem:call` granted beside the other
  system caps in `session/credentials.rs`.
- **`ui/`** — `lib/system/system.types.ts` (+`SubsystemDetail`), `system.api.ts` (`systemSubsystem(id)`),
  `lib/ipc/http.ts` (`system_subsystem` → `GET /system/subsystem/{id}`), `lib/session/admin-caps.ts`
  (`CAP.systemSubsystem`). New `features/system/SubsystemDetailSheet.tsx` (shadcn `Sheet` drawer:
  health, group, role, all metrics, and the bus peer/router zid lists) + `useSubsystemDetail.ts` (loads
  on open, read-only). `SystemView.tsx` now makes **every** card a control — page cards navigate
  (`open <id>`, ↗), no-page cards open the sheet (`subsystem <id>`, +).

**Decisions + the alternative rejected.**

- **A side drawer (`Sheet`), not a separate route or an inline expanding panel.** A route would make a
  read-only console feel like navigation away from the map (and need its own nav/cap plumbing); an
  inline panel would push the grid around on a phone. A `Sheet` keeps the grid in place, is shadcn-first
  and responsive out of the box, and reads as "peek at this subsystem", which is exactly the altitude.
- **One new verb (`system.subsystem`) reusing `collect_services`, not a per-subsystem read path.**
  Mirrors the dbview/system shape exactly (one verb/file, single gate, opaque error) and guarantees the
  detail card is byte-identical to the grid card (same gather) — rejected adding bespoke per-id readers,
  which would let the detail drift from the grid.
- **Unknown id → opaque `Denied`, not `NotFound`.** Keeps the no-existence-signal posture of the rest of
  the map; an id probe learns nothing.

**Tests (green output pasted).**

- Rust — `system_map_test.rs` grew **5 → 9** (real `Node`, real seeds): subsystem returns the right
  card + a `{}` extra for gateway and zid-string arrays for bus; unknown/blank id is opaque (not a
  panic); **cap-deny** (no cap, and overview/topology do NOT grant subsystem); **2-ws isolation** (B's
  outbox detail never reflects A's dead-letter).

  ```
  cargo test -p lb-bus -p lb-host -p lb-role-gateway
  … running 9 tests (system_map_test) … test result: ok. 9 passed; 0 failed
  (lb-bus 2/2, lb-role-gateway suites all ok)
  ```

- UI — `SystemView.gateway.test.tsx` grew **7 → 9** (real spawned gateway): clicking the no-page `bus`
  card opens the detail sheet showing the live peer/router zid lists (count agrees with the card's own
  `peers` metric — both read one `system.subsystem` snapshot); a ≤360px narrow-viewport sheet
  responsive smoke (no horizontal overflow).

  ```
  pnpm test:gateway
  ✓ src/features/system/SystemView.gateway.test.tsx (9 tests)
  Test Files  23 passed (23)   Tests  82 passed (82)
  ```

- `npx tsc --noEmit` 0 errors; `pnpm lint` 0 errors (pre-existing warnings only, none in new files).

**Debugging.** None needed. One test-authoring correction (not a product bug): an initial Rust assertion
demanded the bus card's `peers` metric exactly equal the `peer_zids` length, and a UI assertion expected
`peers (0)` on a "solo" node. Both are wrong against the **shared in-proc test mesh**, where sibling test
nodes are real peers and the live count drifts between two independent `session.info()` reads. Relaxed to
assert the invariant that actually holds (extra is present zid-string arrays for `bus`, `{}` elsewhere;
the UI list count matches the card's own metric read within the same snapshot). No `docs/debugging/`
entry — no product defect, no regression to guard.

**Build/test caveat.** Unlike Session 2, the full `cargo build --workspace` + `cargo test --workspace`
were **green in-tree this session** — the concurrent `lb-runtime` refactor that previously blocked the
whole-workspace build had landed/compiled by the time this slice built, so no clean-worktree workaround
was needed.

**Public/scope updates.** `docs/public/system-map/system-map.md` gains the third verb + its JSON shape,
the cap, the `/system/subsystem/{id}` route, the detail view, and the bus peer list. The scope's "deep
liveness probes / control actions inline" open question is refreshed (the bus now exposes its live peer
*identities*, not just a count; control-actions-inline stays deferred — still read-only). `STATUS.md`
system-map row gets a Session 3 note.

**Follow-ups (unchanged).** Live `system.watch` feed; a pub→sub echo probe for a true round-trip
liveness; typed per-crate `status()` to retire table-name matching; control actions inline (still
deferred — read-only by design).
