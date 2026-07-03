# Frontend scope — dashboard query cache & call de-duplication

Status: **shipped** (see `public/frontend/dashboard.md` → "Read cache & call de-duplication";
session `sessions/frontend/dashboard-query-cache-session.md`). Open questions resolved below.

The dashboard fires far too many gateway/MCP calls. Every data hook is a hand-rolled
`useEffect` + `invoke("mcp_call", …)` with its own private `useState` — there is **no
request cache, no in-flight de-duplication, and no shared result** anywhere on the
surface. Opening a dashboard, clicking **Edit panel**, and typing in the query each set
off bursts of redundant round-trips: the same `viz.query` runs 2–3× for one draft panel
on every keystroke, the whole source-picker bundle (6 list calls + one `flows.get` **per
flow**) is fetched twice, `datasource.list` up to 3×, and N cells on one series/flow each
do their own read + SSE. We want to introduce **one caching/de-dup layer** so identical
reads collapse to a single call, shared data is fetched once per page, and cheap settings
(chart/field config, source lists) survive while the user is on the page and are dropped
when they leave. Adopt `@tanstack/react-query` as that layer and route dashboard reads
through it.

## Goals

- **Adopt `@tanstack/react-query`** as the dashboard's read/cache layer, with a
  `QueryClient` scoped to the dashboard route so the cache lives for the visit and is
  torn down on leave (the "cache while on the page" the user asked for).
- **Collapse the 2–3× `viz.query` per draft panel** (probe + preview + plot) to **one**
  shared query per `{resolved targets+transforms+fieldConfig, scope}`, and stop
  title/layout/option edits from triggering a data refetch (narrow the query key off the
  whole-panel JSON).
- **Share the source-picker bundle** between the page-level and editor instances (one
  fetch per workspace, not two), which also removes one of the duplicate `datasource.list`
  calls.
- **De-dup per-widget reads across cells**: N cells on the same series share one
  `series.read` (and, where possible, one SSE); N cells on one flow share one
  `flows.node_state` read (it already returns the whole flow — slice client-side).
- **In-flight coalescing** so two components requesting the same `{tool,args}` in the same
  tick issue a single network call.
- Net: a measurable drop in call count on page-open / edit-panel / query-edit (the three
  paths the user flagged), with **no behavioural change** to what renders.

## Non-goals

- **No server/host changes.** This is a client caching layer over the existing MCP verbs;
  `viz.query`, `dashboard.*`, `series.*`, `flows.*`, `datasource.*` are untouched. No new
  tools, no new gateway routes, no capability changes.
- **Not the live/watch (SSE) transport.** Streaming stays on the shipped series/bus SSE
  (`series.watch`/`bus.watch` in `widgetBridge.ts`). We may share *subscribers* to one
  stream, but we do not move motion into the query cache (state vs motion, README §3.3).
- **No global rewrite of every hook to react-query at once.** Scope is the dashboard
  surface (`ui/src/features/dashboard/**` + the `@nube/source-picker` shell adapter). Other
  features migrate later if this proves out.
- **No offline/persisted cache.** In-memory only, dropped on route leave. No
  `localStorage`/IndexedDB persistence.
- Not a redesign of the panel editor or the source picker UX — pure plumbing under them.

## Intent / approach

**Route every dashboard *read* through react-query; keep every *write* and every *stream*
exactly where it is.** react-query gives us four things we hand-roll badly today —
in-flight de-duplication, a keyed result cache with `staleTime`, subscriber sharing, and
lifecycle-scoped teardown — for free and battle-tested. We already ship
`@tanstack/react-router`, so the ecosystem and mental model are in-house.

Concretely:

1. **A `QueryClientProvider` scoped to the dashboard route** (not the app root), so the
   cache's lifetime is "while the user is on the dashboard page". `staleTime` tuned per
   read class: source lists / datasource list / flow roster get a generous stale window
   (rarely change during a visit); `viz.query` results get a short one keyed on the
   refresh tick. Leaving the route unmounts the provider and drops the cache — the exact
   "cache while here, clear on leave" the user wants.

2. **One `useVizQuery` shared by probe + preview + plot.** Today three independent
   `usePanelData` instances (`PanelEditor.tsx:116`, `PreviewPane.tsx:52`,
   `PlotAxesTab.tsx:32`) each mount their own debounced call for the *same* draft. Move the
   fetch into a `useQuery` keyed on the **resolved query spec** — `{targets, transforms,
   fieldConfig, scope, refreshTick}` — *not* the whole panel JSON. All three instances
   share one cache entry; a title/option/layout edit no longer re-keys, so it no longer
   refetches. The 200ms debounce becomes a debounce on the *key input*, not per-instance.

3. **Source picker as a shared query.** `useSourcePicker(ws)` becomes a `useQuery` keyed on
   `["source-picker", ws]`. The page-level (`DashboardView.tsx:44`) and editor
   (`QueryTab.tsx:65`) instances read the same cache entry → one bundle fetch per
   workspace, not two. `useDatasourceList` reads the same `["datasource.list", ws]` key the
   bundle populates, so `datasource.list` fires once.

4. **Per-source de-dup across cells.** `series.read` → `useQuery(["series.read", series,
   args])`; `flows.node_state` → `useQuery(["flows.node_state", flowId, refreshTick])` with
   client-side slicing per cell. N cells on one source now share one cache entry. SSE
   subscriber-sharing (one `EventSource` per series/subject, fan-out to N cells) is a
   companion win layered in the bridge — kept out of the query cache but sharing the same
   "one upstream, many readers" shape.

5. **The de-dup chokepoint is `widgetBridge.ts`.** Every read already funnels through
   `makeWidgetBridge().call(tool, args)` → `invoke("mcp_call", …)`. react-query's
   `queryFn` wraps `bridge.call`; the bridge's local scope-filter and the host's
   server-side re-check are **unchanged** (defense in depth holds — the cache never sees the
   token, never bypasses the capability check).

**Alternative considered — a hand-rolled ~100-line keyed cache in `widgetBridge.ts`** (no
new dependency). Rejected: we'd be reimplementing exactly what react-query already does
(stale-time, GC, in-flight coalescing, subscriber ref-counting, devtools), and the "AI
wrote bespoke plumbing that drifted messy over time" is *how we got here*. A well-known,
documented library is more legible to the next agent and to a human reviewer than another
in-house cache. The cost is one dependency and a `QueryClientProvider` — cheap and
reversible.

## How it fits the core

- **Tenancy / isolation:** every query key is **prefixed by `ws`** (workspace). A
  workspace switch changes the key → different cache entries; no cross-workspace bleed. The
  host still re-checks the workspace from the token on every `mcp_call` regardless of the
  cache. Isolation is tested (below).
- **Capabilities:** unchanged. The cache wraps `bridge.call`, which enforces `cell.tools ∩
  grant` locally and the host re-checks the capability server-side on **every** call. A
  denied read throws and is cached as an honest denied/empty state (as
  `useVizQuery` already does), never a fabricated value. Deny-path tested.
- **Placement:** **client-only** (browser + Tauri shell alike). No node code changes, so
  symmetric-nodes is trivially preserved — there is no `if cloud` to introduce.
- **MCP surface:** **none added or changed.** This is a consumer-side optimization over the
  existing read verbs (`viz.query`, `dashboard.get/list`, `series.read`, `flows.node_state`,
  `datasource.list`, `series.list`, `ext.list`, `flows.list/get/nodes`). API-shape verbs are
  all pre-existing **get/list** and **live-feed**; no new CRUD, no batch, no job.
- **Data (SurrealDB):** N/A — no records touched. Reads hit the same tables through the
  same verbs; caching is purely in-browser.
- **Bus (Zenoh):** N/A for the cache. The **watch** paths keep riding the shipped series/bus
  SSE; subscriber-sharing may reduce the number of `EventSource` connections but adds no new
  subject or message class (state vs motion preserved).
- **Sync / authority:** node stays authoritative; the cache is a short-lived client
  read-through with a `staleTime`, dropped on route leave. No offline behaviour.
- **Secrets:** N/A — the token never enters `args` or the cache key; it stays in the
  shell/gateway seam exactly as today.

## Example flow

Editing a panel's query, after this scope:

1. User opens a dashboard. `DashboardView` mounts under the route's `QueryClientProvider`.
   `useSourcePicker(ws)` populates `["source-picker", ws]` **once**; `useDatasourceList`
   reads the same cached `datasource.list`. Each cell's `viz.query` populates its own
   `["viz.query", spec, scope, tick]` entry.
2. User clicks **Edit panel**. `QueryTab` mounts and calls `useSourcePicker(ws)` again — it
   **hits the warm cache** (`staleTime` not elapsed): **zero** new network calls for the
   source bundle or `datasource.list`.
3. The editor's probe (`PanelEditor`), preview (`PreviewPane`), and — if open — the plot
   tab (`PlotAxesTab`) all call the shared `useVizQuery(draft, scope)`. They resolve to
   **one** `["viz.query", …]` entry → **one** gateway round-trip, shared three ways.
4. User types in the SQL/federation box. The **query spec** changes → the key changes →
   **one** debounced `viz.query` fires and all three consumers update from it. Editing the
   panel **title** does *not* change the query key → **no** refetch.
5. User closes the editor and navigates away from the dashboard. The route unmounts, the
   `QueryClientProvider` tears down, and the whole cache (source lists, chart/field config,
   query results) is dropped — as asked.

## Testing plan

Against the **real** spawned gateway (`pnpm test:gateway`, `vitest.gateway.config.ts`) —
seed real dashboard/series/flow/datasource records into the real store (CLAUDE §9 / testing
§0; no `*.fake.ts`, no mock backend). Mandatory categories that apply:

- **Capability deny-test (required):** with a session grant lacking the read cap, the cached
  query resolves to the honest **denied/empty** state and **never** a fabricated value — and
  a denied result is not silently reused across a workspace where the cap differs.
- **Workspace-isolation (required):** two workspaces with same-named series/flows/datasources.
  Prime the cache in workspace A, switch to B → B reads B's data (keys are `ws`-prefixed);
  no A value bleeds into B. Switch back → A still isolated.
- **De-dup assertions (the whole point):** instrument `invoke`/`bridge.call` call counts on
  the real-gateway harness and assert:
  - opening a dashboard with N cells on one series issues **one** `series.read`, not N;
  - opening the editor issues **zero** extra source-picker/`datasource.list` calls (warm
    cache);
  - typing in the query issues **one** `viz.query` per debounced spec change (not 2–3), and
    a **title-only** edit issues **none**;
  - two cells on one flow issue **one** `flows.node_state`.
- **No behavioural regression:** existing dashboard/editor render tests stay green — same
  rows, same rendered widgets, same denied states.
- **Hot-reload / stateless:** N/A to the node (client-only change), but confirm the cache is
  purely in-memory and route-scoped (nothing persisted).

## Risks & hard problems

- **Query-key design is the whole ballgame.** Too coarse (whole-panel JSON, as today) →
  spurious refetches; too fine or unstable (object identity, key reordering, `undefined`
  leaves) → cache misses that look like the current behaviour. The `viz.query` key must be a
  **canonicalised** `{targets, transforms, fieldConfig, scope, tick}`, stable across
  unrelated edits. Get this wrong and the scope delivers nothing.
- **`staleTime` vs freshness.** Source lists cached too long could hide a newly-created
  series/flow mid-session. Pick a stale window that de-dups the burst but still refreshes on
  a sensible trigger (workspace switch, explicit refresh, editor-open invalidation where a
  fresh list genuinely matters).
- **Debounce moves.** Today each `useVizQuery` debounces independently; consolidating to one
  shared query means debouncing the **key input**. A regression here could make the editor
  feel laggy or, worse, fire on every keystroke — assert the debounced-call-count test.
- **SSE subscriber-sharing is the trickiest piece.** Ref-counting one `EventSource` across N
  cells with correct teardown on the *last* unmount is easy to leak. Keep it out of the query
  cache; if it proves fiddly, ship the query-cache de-dup first and land subscriber-sharing
  as a follow-up slice (it's the smaller win).
- **Provider placement.** The `QueryClientProvider` must scope to the dashboard route, not
  the app root — root placement would leak the cache across the whole app and defeat the
  "clear on leave" requirement.

## Open questions

- **Provider scope:** dashboard route only (recommended, matches "clear on leave"), or a
  shared app-level client with route-scoped cache-clearing? Decide before wiring.
- **`staleTime` per read class:** what windows for (a) source/datasource/flow lists, (b)
  `viz.query` results, (c) `flows.node_state`? Propose: lists ~30–60s, query results keyed on
  refresh tick (effectively "until next tick"), node-state on the tick.
- **Ship SSE subscriber-sharing in this slice or defer it?** Recommendation: defer to a
  follow-up if it adds risk; the query-cache de-dup is the majority of the flagged calls.
- **How much of `@nube/source-picker` moves?** Does the package hook itself adopt react-query
  (making it a package dependency), or does only the **shell adapter** (`builder/useSourcePicker.ts`)
  wrap the package loaders in `useQuery`? Recommendation: wrap in the shell adapter so the
  package stays framework-light and reusable from an extension bridge.
- **Devtools:** include `@tanstack/react-query-devtools` in dev builds for verifying de-dup?
  (Low-cost, high-signal for this exact work.)

## Related

- Prior scopes: `scope/frontend/dashboard-scope.md`, `dashboard-widgets-scope.md`,
  `widget-builder-scope.md`, `dashboard/widget-config-vars-scope.md`,
  `dashboard/source-picker-package-scope.md`, `dashboard/viz/` (the one viz bridge).
- Shipped truth: `public/frontend/dashboard.md`, `public/frontend/widget-kit.md`.
- Core principles: README `§3` (state vs motion, capability-first, workspace wall),
  `§6.13` (gateway SSE routes), `docs/FILE-LAYOUT.md` (one hook/responsibility per file —
  the new query hooks obey it).
- Key files this touches: `ui/src/features/dashboard/builder/widgetBridge.ts` (the read
  chokepoint), `builder/useSourcePicker.ts`, `builder/QueryTab.tsx`, `builder/PanelEditor.tsx`,
  `builder/PreviewPane.tsx`, `builder/PlotAxesTab.tsx`, the `usePanelData`/`useVizQuery`/
  `useSeries`/`useFlowNodeValue` hooks, `DashboardView.tsx`, `useDatasourceList.ts`.
- Skill doc: **N/A** — no new agent-/API-drivable surface (no MCP verbs, no gateway routes;
  a client-side caching layer). The existing dashboard skill, if any, is unaffected.
