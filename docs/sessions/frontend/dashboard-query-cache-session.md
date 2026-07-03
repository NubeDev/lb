# Session — dashboard query cache & call de-duplication

Scope: `docs/scope/frontend/dashboard-query-cache-scope.md`. Branch: `master`.

## The ask

The dashboard fired far too many gateway/MCP calls: every data hook was a hand-rolled
`useEffect` + `invoke("mcp_call", …)` with its own private `useState` — no request cache, no in-flight
de-dup, no shared result. Opening a dashboard, clicking **Edit panel**, and typing in a query each set
off bursts of redundant round-trips (the same `viz.query` 2–3× per draft panel per keystroke; the source
picker bundle + `datasource.list` fetched twice; N cells on one series/flow each doing their own read).
Adopt `@tanstack/react-query` as ONE caching/de-dup layer for dashboard **reads**, scoped to the
dashboard visit, and route the flagged reads through it — with **no behavioural change** to what renders.

## What shipped

A `features/dashboard/cache/` slice + rewired read hooks:

- **`dashboardQueryClient.ts`** — the per-visit `QueryClient` factory. `retry:false` (a denied read must
  surface as an honest denied state, never be retried into a spurious success), `refetchOnWindowFocus:false`
  (the refresh **tick** is the freshness signal, carried in the key), `LIST_STALE_MS = 30_000`.
- **`DashboardQueryProvider.tsx` → `DashboardCacheProvider`** — provides the per-mount `QueryClient` **and**
  the current `ws` (via `DashboardWsContext`) to the dashboard subtree. Mounted by **`DashboardView`**
  (self-wraps its body, keyed on `ws`) so the real route AND the gateway tests (which render `DashboardView`
  directly) get the cache with no extra wiring; **and by `ResponseView`** (the channel `rich_result` reuses
  the shipped panel renderer off-route, so it needs the same boundary). Leaving the dashboard route unmounts
  `DashboardView` → the client (and its cache) is dropped — the scope's "cache while here, clear on leave".
- **`queryKeys.ts`** — canonical, **ws-prefixed** keys. `canon()` sorts object keys + drops `undefined` so
  an unrelated edit doesn't re-key. The `viz.query` key is the **resolved spec** `{sources, transformations,
  fieldConfig, source, scope, tick}` — NOT the whole-panel JSON (the coarse key that caused the spurious
  refetches). `flows.node_state` keyed on `(ws, flowId, tick)`; `series.read` on `(ws, binding)`.
- **`useVizQuery.ts`** — rewritten on `useQuery`, keyed on the debounced canonical spec. The three editor
  consumers (probe/preview/plot) that mount it for one draft now share ONE cache entry → ONE round-trip; a
  title/layout/option edit no longer re-keys → no refetch. The 200ms debounce moved to the **key input**
  (`useDebounced`) — one debounce, not one per consumer.
- **`useSourcePicker.ts` (shell adapter)** — now a `useQuery` keyed `["source-picker", ws]`, so the
  page-level and editor instances share one bundle fetch. The package stays framework-light: a new PURE
  `loadSourcePicker(loaders)` was extracted in `@nube/source-picker` (the package hook now delegates to it);
  only the shell adapter adopts react-query (the scope's recommended answer to "how much of the package moves").
- **`datasourceListQuery.ts`** — one `["datasource.list", ws]` definition. `useDatasourceList` reads it via
  `useQuery`; the source-picker's `listDatasources` loader routes through the **same** key via
  `fetchDatasourceList` → `datasource.list` fires once per ws.
- **`useFlowNodeValue.ts`** — the whole-flow `flows.node_state` read is the shared entry (keyed on
  `(ws, flow, tick)`, not node/port); each cell slices its own node/port/path **client-side** from the
  shared result → N cells on one flow = one read.
- **`useSeries.ts`** — the resolve + `series.read` backfill (STATE) is cached & de-duped; the live SSE tail
  (MOTION) stays **outside** the cache, folded over the shared backfill locally (state vs motion, README §3.3).

## Decisions (closing the scope's open questions)

- **Provider scope:** dashboard route only, via `DashboardView` self-wrapping (not the app root — root
  placement would leak the cache and defeat "clear on leave"). Channel `ResponseView` gets its own boundary
  because it reuses the panel renderer off-route.
- **`staleTime`:** lists (source picker / datasource / flow roster) = 30s; `viz.query` + `flows.node_state`
  keyed on the refresh **tick** (effectively "fresh until the next tick") — no time-based staleness needed.
- **SSE subscriber-sharing:** DEFERRED (the scope's recommendation) — the query-cache de-dup is the majority
  of the flagged calls; sharing one `EventSource` across N cells is the smaller, fiddlier follow-up win.
- **Devtools:** not added (keep the dep surface minimal; can be added in dev builds later).

## Testing

`queryCache.gateway.test.tsx` (real spawned gateway, real seeded series/flow records — CLAUDE §9 / testing
§0, no fake backend) asserts the scope's mandatory categories by instrumenting the `invoke` seam (the spy
**delegates** to the real transport — observe, never fake):

- **De-dup:** 3 stat cells sharing one `viz.query` spec (different ids + titles) → **one** `mcp_call{viz.query}`,
  all three render the real seeded value; 2 cells on one flow → **one** `flows_node_state`.
- **Workspace-isolation:** same-named `iso.temp` seeded `11` in ws-A / `22` in ws-B; ws-B (own provider,
  ws-prefixed key) reads `22`, no ws-A bleed.
- **Deny:** a session with `series.find` but not `viz.query` → the panel's honest **denied** message, **no**
  stat value (never a fabricated number).

Suites: UI unit `pnpm test` **426 passed**; UI real-gateway `pnpm test:gateway` — all dashboard/channel
tests green. Two unrelated failures (`SystemView` bus-peer-count, `sqlSource` e2e render) reproduce in the
full parallel run but pass in isolation and are **pre-existing** (verified: `SystemView` uses no cache hook
and fails with the import/wrap removed; `sqlSource` fails on clean master) — cross-test live-mesh flakes,
not caused by this change.

## Build hygiene

Adding `@tanstack/react-query` pulled a transitive `@types/react@19` that collided with the app's React 18
JSX types (`bigint`-in-ReactNode errors on every icon). Pinned `@types/react`/`@types/react-dom` to 18 in
`pnpm-workspace.yaml` `overrides` — one type version, coherent JSX. The `@nube/source-picker` package was
rebuilt (`pnpm build`) so `loadSourcePicker` is in its `dist` (the ui resolves the package from `dist`).

## Follow-ups (not in this slice)

- **SSE subscriber-sharing** — one `EventSource` per series/subject fanned to N cells (deferred above).
- **`<SourcePicker>` component consolidation** — orthogonal cleanup surfaced mid-session: the package ships
  a `<SourcePicker>` component (with `PickerGroup` + `DEFAULT_GROUPS`) that nothing imports; three call
  sites (`WidgetBuilder.tsx`, `QueryTab.tsx`, `VariableEditor.tsx`) re-roll it with duplicated `PickerGroup`
  definitions + hardcoded group lists. The MODEL is fully shared (no drift risk); this is an incomplete
  *component* migration, ~40 lines of duplicated picker UI. Contained refactor, own session — sequenced
  AFTER this cache work because it touches the same overlapping files.
