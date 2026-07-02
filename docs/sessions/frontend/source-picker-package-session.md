# Session — extract the source picker into `@nube/source-picker` (package + dashboard refactor)

- Topic: frontend / dashboard (reusable source picker)
- Status: **in-progress** — package built + dashboard migrated (parity green); thecrew consumer next.
- Scope: [`docs/scope/frontend/dashboard/source-picker-package-scope.md`](../../scope/frontend/dashboard/source-picker-package-scope.md)
- Branch: `crew-v2` (nothing committed).

## The ask

Let a user (and an AI) select a value/source from the **DB / datasources / Zenoh (live series) /
flows** — the SAME way the dashboard does — from surfaces other than the dashboard (first: the
`thecrew` graphics-canvas extension). The machinery already existed but was trapped inside
`ui/src/features/dashboard/`. Extract it into a reusable, transport-agnostic package.

## Part 1 — the package (`packages/source-picker`, `@nube/source-picker`) — SHIPPED standalone

Mirrors `@nube/panel`/`@nube/nav-rail` (pure, props-driven, React peer dep, ESM+CJS+dts+scoped CSS).
Three adoptable layers:
- **Model** (`sourcePicker.ts`) — `SourceEntry` + `buildSourceEntries({series,extensions,flows,descriptors})`
  + all group builders (series/live/sql/extension/widget/flows) + `widgetIdOf` + `selectionOf`. Pure.
- **Loader hook** (`useSourcePicker.ts`) — takes an INJECTED `SourceLoaders` seam; deny-tolerant
  (a rejected loader → that group is empty); keyed on `ws`. **Reads `loaders` through a ref** so an
  unmemoized `loaders` object per render does NOT infinite-loop (caught via an OOM in the package tests
  — a real robustness fix, since hosts WILL pass literals).
- **UI** (`SourcePicker.tsx`) — the props-driven grouped `<select>` firing `onSelect(SourceSelection)`;
  self-themed `--sp-*` tokens scoped to `.sp-root`.
- **Types** (`types.ts`) — `Source`/`Action`/`SourceSelection` + the `SourceLoaders` interface + the
  row shapes (mirror the node's wire records).

The package imports **no `@/` alias and no transport** — that's what makes one picker work from both the
shell (gateway/Tauri) and an extension (its bridge). Tests: 16/16; `tsc` clean; builds
6.1KB ESM / 4.4KB CJS / 0.5KB CSS + rolled-up types, React externalized.

## Part 2 — dashboard refactor onto the package (parity) — SHIPPED

Kept every consumer's import path unchanged via two SHIM files, so the diff is small and low-risk:
- `builder/sourcePicker.ts` → re-exports the package model; keeps the dashboard's POSITIONAL
  `buildSourceEntries(series, rows, flows?, descriptors?)` signature as a thin adapter over the package's
  object form (so all call sites + tests are untouched).
- `builder/useSourcePicker.ts` → the **shell adapter**: builds `SourceLoaders` from the shipped
  `@/lib/*` clients (`listSeries`/`listExtensions`/`listFlows`/`getFlow`/`listFlowNodes`/`listDatasources`)
  and delegates to the package hook. This is the ONE place `@/lib/*` meets the package. `installed` is
  re-asserted to the shell's fuller `ExtRow` (the runtime value IS the shell row; the package types it as
  its structural subset).
- `builder/ExtWidget.tsx` → imports `widgetIdOf` from the package (dropped its local copy) — one slug
  function shared by picker + renderer, never two that drift.
- `ui/package.json` → `@nube/source-picker: workspace:*`.

**Parity proof (no behavior change):**
- Dashboard unit: 129/129 (incl. widgetBuilder/flowsPicker/QueryTab/FlowsQuerySection/cellEditorState).
- Dashboard gateway (real spawned node): `widgetBuilder.gateway` 17/17, `panelEditor.gateway` 6/6,
  `flowsPanelEditor.gateway` 4/4, `FlowDashboardBinding.gateway` + `DashboardView.gateway` (see note).
- thecrew unit still 66/66 (`ExtWidget` slug change is transparent).
- `tsc` clean (only the 2 pre-existing `FlowsCanvas.gateway.test` errors remain); shell `vite build` OK.

**Pre-existing failure (NOT mine):** `DashboardView.gateway` → "renders a timeseries panel … full
option surface" fails on `getByLabelText("tab Field")` — reproduced on the STASHED clean tree, so it
predates this work (the panel editor's Field tab was renamed/removed by other work). Left as-is.

## Zero core additions

Pure frontend. The package CONSUMES the same shipped reads via injected loaders; no new verb/cap/table/WIT.

## Not done yet (this session continues)

- **thecrew consumer:** wire a bridge-backed `SourceLoaders` into `thecrew/ui` so a scene shape can bind
  to a db/series/live/flow source through the SAME picker (scene `bind` stays `{channel}` for the first
  cut per the scope decision; the picker fills the series name).
- Promote to `public/frontend/dashboard.md` on ship.
