# Session — extract the source picker into `@nube/source-picker` (package + dashboard refactor)

- Topic: frontend / dashboard (reusable source picker)
- Status: **shipped** — package built, dashboard migrated (parity green), thecrew consumer wired + LIVE.
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

## Part 3 — thecrew consumer (the reuse payoff) — SHIPPED + LIVE

Wired the SAME package into the standalone `thecrew/ui`, reaching the node through the extension
**bridge** instead of `@/lib/*` — proving transport-agnostic reuse across the shell/extension boundary.

- **Dep:** `@nube/source-picker` as a `file:` dep (thecrew builds `--ignore-workspace`, so no
  `workspace:*`); the package's built `dist/` + `exports` map resolves.
- **Bridge-backed loaders** (`bridge/source-loaders.ts`): `bridgeSourceLoaders(bridge)` implements
  `listSeries` over `bridge.call("series.list")`. Only `series.list` is wired (thecrew's grant covers
  `series.*`/`assets.*`); the other loaders are omitted → their picker groups are simply absent (the
  package's capability-scoped contract). A scene binds to series channels, so that's the whole surface.
- **Manifest:** added `mcp:series.list:call` to `[capabilities].request` + the `[ui]` scope — a shipped
  verb the picker needs to DISCOVER series (not a new verb/cap; consumer request, zero core additions).
- **Loaders context** (`data/use-source-loaders.ts`): carries the loaders from the mount shell (which
  holds the bridge) to the prop-less `App` tree; `ScenePage` provides it around `<App/>`.
- **PropertyRail bind picker** (`editor/PropertyRail.tsx`): replaced the old `<select>` over
  `source.channels()` (a closed loop — only channels ALREADY bound in the scene) with the reusable
  `<SourcePicker>` over `useSourcePicker(loaders, ...)`. It now lists EVERY workspace series; a
  `series`/`live` selection's `source.args.series` becomes the bind channel. `bind` stays `{channel}`
  (scope decision). CSS: the picker's scoped `.sp-*` rules aliased to thecrew's glass tokens in
  `styles.css` (thecrew injects only its own `?inline` stylesheet).

**Live verification (real node in-mem :8080, built shell :4173, thecrew republished with the new
manifest + reseeded):** `ui/e2e/thecrew-bind-picker.spec.ts` (**1/1**) — open Graphics → AHU-1 → select
SF-1 → the `speed` bind slot's picker lists workspace series discovered over the bridge (incl.
`ahu1.oad.position`, NOT bound to `speed` — proving discovery, not the old bound-only loop) → pick
`ahu1.rat` → the store's bind channel updates. Screenshot: `docs/shots/scene-bind-picker-live.png` (the
rail's three slots as pickers, live values beside each). The page + widget e2e still 2/2.

Tests: thecrew unit **69/69** (+3 `PropertyRail.test.tsx`, driven through the real `mountPage`+stub
bridge — no RTL dep, matching `mount.test.tsx`'s raw-DOM style); package 16/16; dashboard 124/124 (parity
intact); all `tsc` clean.

## Status

Both consumers shipped + verified. Promote to `public/frontend/dashboard.md` when the whole slice is
called done. Follow-up (filed, not this session): widen scene `bind` beyond `{channel}` to the full
`{tool,args}` source vocab (`scene-source-binding`).
