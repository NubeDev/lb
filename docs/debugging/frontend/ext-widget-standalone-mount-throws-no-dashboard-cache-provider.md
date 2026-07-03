# ExtWidget throws "no DashboardCacheProvider in tree" when mounted standalone

Area: frontend / dashboard ext-widget (frames-in). Date: 2026-07-03.

## Symptom

After wiring frames-in, `ExtWidget.test.tsx` (5 tests) failed with:

```
Error: useDashboardWs: no DashboardCacheProvider in tree
 ❯ useDashboardWs src/features/dashboard/cache/useDashboardWs.ts:18
 ❯ useVizFrames src/features/dashboard/builder/useVizFrames.ts:47
 ❯ ExtWidget src/features/dashboard/builder/ExtWidget.tsx:111
```

The existing ExtWidget unit tests render `<ExtWidget … />` **standalone** (no dashboard page, no
`DashboardCacheProvider`) to test the v2 tile lifecycle (private-slot mount, teardown-once). The new
frames-in code made `ExtWidget` call `useVizFrames` unconditionally (rules-of-hooks), and `useVizFrames`
called `useDashboardWs()`, which **throws** by design when there is no provider — a guard meant to stop a
dashboard read hook from silently keying under an unscoped `""` workspace.

## Cause

`useDashboardWs` is correctly strict for the built-in read hooks (they only ever render inside the
dashboard page). But an ext widget is a legitimately BROADER consumer: a **v2 self-fetching tile** can
mount outside a dashboard (it reads through its own bridge, needs no frames), so requiring the cache
provider to even MOUNT the widget was wrong. The strict throw is right for `useVizQuery`; it is wrong as
a hard precondition for `useVizFrames`.

## Fix

- Added `useDashboardWsOptional()` (`cache/useDashboardWs.ts`) — returns `null` outside a provider
  instead of throwing. The strict `useDashboardWs()` is unchanged (still the right guard for built-in
  read hooks).
- `useVizFrames` now reads the optional ws and only enables its `useQuery` when `ws !== null`
  (`enabled = hasTarget && ws !== null`), passing a throwaway `STANDALONE_CLIENT` when there is no
  provider so `useQuery` satisfies rules-of-hooks without a `QueryClientProvider`. When a provider IS in
  the tree, it passes `undefined` so context resolves the SAME client → the dedup with `useVizQuery`
  holds. Without a ws: no fetch, empty frames — a v2 tile is unaffected, a data tile only resolves under
  the dashboard/channel cache that supplies the ws (no unscoped key, no cross-ws bleed).

## Regression coverage

- `ExtWidget.test.tsx` (the 5 standalone tests) passes again — `pnpm test` **426 passed**.
- `framesIn.gateway.test.tsx` "v2 compat" asserts a v2 tile resolves NO frames under the v3 shell (the
  self-fetching path is untouched), and "workspace isolation" asserts a data tile keys per-ws.

## Lesson

A strict "you must be inside provider X" hook guard is correct for a narrow consumer but becomes a bug
when a broader consumer legitimately renders outside X. Prefer an explicit optional variant over
loosening the strict one — the strict guard still protects the hooks that need it.
