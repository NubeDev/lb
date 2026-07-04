# Data Studio editing UX — query→see-data, fast (session)

- Date: 2026-07-04
- Scope: ../../../scope/frontend/dashboard/data-studio-ux-scope.md
- Stage: post-S8 (frontend polish over the shipped viz layer)
- Status: done

## Goal
Make the Data Studio build/test loop Grafana-Explore-grade: an honest query-status line
(rows/frames/duration/error/why-empty instead of a silent "no data yet"), clear Run
semantics, a searchable source picker, and — the load-bearing one — **edit the chart and
refresh with the data that's already there, without re-querying the datasource**.

## What changed

**Backend — `viz.query` frames-in (compute-only) mode.**
- `rust/crates/host/src/viz/query.rs` — `viz_query` now branches: when the panel carries an
  inline `frames` array it runs the transform pipeline over THOSE and resolves no source
  (`panel_inline_frames`). Same verb, same `mcp:viz.query:call` gate; a frames-in request
  reaches no gated read (it resolves nothing) and each posted frame is truncated to the
  per-frame budget. The pipeline stays the ONE server-side impl (rule 9 — no client mirror).
- Test `frames_in_shapes_without_resolving_a_source` in
  `rust/crates/host/tests/viz_query_test.rs`: a caller holding `viz.query` but NOT
  `store.query` still gets shaped rows (proves no source is touched) + parity with the
  store-backed pipeline result.

**Frontend — fetch/shape split (`edit-without-requery`).**
- `useVizQuery.ts` rewritten into two chained react-query calls: a **fetch** keyed on
  `{sources, source, scope, tick}` (raw frames, empty pipeline) and a **shape** keyed on
  `{framesHash, transformations, fieldConfig}` (compute-only `viz.query` over the cached raw
  frames). A transform/field-config edit re-keys only the shape → no datasource hit; a
  source/SQL/time-range edit (or Run) re-keys the fetch. No pipeline → the raw frames are
  used directly (no second round-trip). Over a ~4 MB frames budget it falls back to a normal
  fetch (status bar says so). New keys `vizFetchKey`/`vizShapeKey` in `cache/queryKeys.ts`.
- **Freeze** ("use current data"): `cache/useFreeze.ts` — an ambient `FreezeProvider` the
  editor wraps its preview subtree in, read by `useVizQuery` (explicit opt wins). While
  frozen the fetch is disabled AND its key is pinned to the last-fetched spec, so a source
  edit reshapes the frozen frames instead of pointing at a never-fetched empty key.
- `SourceState` gained an optional `meta` (`frames`/`ms`/`error`/`source`/`fetchedAt`) —
  renderers ignore it; only the status bar reads it. The live/flow paths tag their `source`.

**Frontend — the honest loop UI.**
- `QueryStatusBar.tsx` — one status line under the preview: running / ok (rows·frames·ms·as
  of) / error-inline / **0-rows-with-range** / **never-ran-say-what's-missing**, plus a
  "shaped from cached data" chip (the visible payoff of the split) and a frozen chip.
- `PreviewToolbar.tsx` — a real **Run/Refresh** button for every datasource (not just
  federation), a **Freeze** toggle, and the table-view inspect toggle (moved off PreviewPane).
- `BuilderPane.tsx` — wires the toolbar + status bar, freeze state, and ⌘/Ctrl+Enter Run.
- Renamed the Data Studio save button **"Apply" → "Save to tab"** (`BuilderTabPane.tsx`) —
  it persists the draft, it was never the thing that fetches.

**Frontend — searchable source picker.**
- `@nube/source-picker`: new `SourceCombobox.tsx` (type-to-filter grouped popover, keyboard
  nav) + scoped `sp-combo-*` CSS. Same model/tokens as the `<select>`, which stays exported.
  Added `onSelectEntry` (raw entry) so a host keying on id (QueryTab — `rules.run` is shared
  across rule entries) isn't forced through the folded selection.
- `QueryTab.tsx` swaps the native `<Select>` source picker for `SourceCombobox`.

## Decisions & alternatives
- **Server stays the one transform impl.** Rejected a client-side transform mirror (rule 9 —
  a hand-written re-implementation of node behavior that would drift). Frames-in reuses the
  exact `lb-viz` pipeline; the client only splits *when* it fetches vs reshapes.
- **Freeze as ambient context, not a threaded prop.** The rendered preview's renderers each
  call `usePanelData` deep inside `WidgetView`; threading `frozen` through a dozen views
  would re-couple the render path. A `FreezeProvider` reaches them with zero renderer edits
  and defaults to unfrozen off the editor (dashboard render path unchanged).
- **Pin the fetch key while frozen.** First cut only set `enabled:false`; a source edit then
  moved the key to a never-fetched (empty) spec and rows dropped to 0. Pinning the last
  unfrozen spec is what makes freeze reshape the frozen data.
- **Prefer the server's `rows`.** The split derives rows from frames, but I kept preferring
  the server's `rows` verbatim (reconstruct from frames only when absent) so a `rows`-only
  responder (a thin stub, a non-frame tool) still resolves — caught by the ResponseView test.

## Tests
- **Backend** (`cargo test -p lb-host --test viz_query_test`): 8 passed incl. the new
  frames-in test + the existing cap-deny and ws-isolation.
- **Real gateway** (`vitest.gateway.config.ts`, `viz.phase3.gateway.test.tsx`): 6 passed —
  new `freeze keeps the fetched rows even when the source changes to an empty series` and
  `a panel with a transform pipeline reports shaped provenance`, plus the existing mandatory
  cap-deny + ws-isolation. No mocks — real spawned node, real ingest/series.read/viz.query.
- **UI unit** (`pnpm test`): 554 passed (added QueryStatusBar ×7; updated the 3 QueryTab
  extension-widget tests to drive the combobox).
- **Package** (`@nube/source-picker`): 29 passed (added SourceCombobox ×5).

```
# backend
test result: ok. 8 passed; 0 failed; 0 ignored  (viz_query_test)
# ui unit
Test Files  91 passed (91)
      Tests  554 passed (554)
# real gateway (viz.phase3)
 ✓ src/features/dashboard/views/viz.phase3.gateway.test.tsx (6 tests)
# package
      Tests  29 passed (29)
```

Mandatory categories: **capability-deny** ✓ (frames-in + panel deny), **workspace-isolation**
✓ (frames-in resolves nothing; ws-B panel invisible to ws-A). Offline/sync, hot-reload: n/a
(read-path editor UX only).

## Debugging
None opened. Two self-inflicted reds caught by existing tests and fixed in-session (the
`rows`-only responder regression via ResponseView; the freeze-key-pin via the new gateway
test) — neither a platform bug, so no `debugging/` entry.

## Public / scope updates
- Promoted the shipped behavior to `public/frontend/dashboard.md` ("Data Studio editing
  loop"). Flipped `data-studio-ux-scope.md` and the dashboard README "not shipped" line to
  shipped. Resolved scope open questions 1 (frames param on `viz.query`, not a sibling verb),
  2 (freeze per-tab), 3 (combobox in the package).

## Skill docs
n/a for the UI. The frames-in `viz.query` mode is API-drivable; its request shape
(inline `frames` → shaped frames) is documented in this session + the scope's MCP-surface
section. No standalone `skills/` entry exists for `viz.query` to update.
