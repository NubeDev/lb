# Dashboard scope — extension widgets over any source (frames-in)

Status: **SHIPPED 2026-07-03** (branch `master`). Promoted to `public/frontend/dashboard.md` → "Extension
widgets over any source — frames-in"; session `sessions/frontend/ext-widget-frames-in-session.md`.
Open questions resolved: transformations run before frames reach the tile (parity with built-ins);
`data` is a **bool** (an enum can come later, additively); the Query tab is NOT hideable per tile in v1
(full parity). Deferred: Part C (`@nube/widget` package extraction) and the `useSceneDocs` cleanup rider
(their own slices — see the session doc). Original ask below.

An extension-shipped `[[widget]]` tile should be a **first-class visualization over the shipped
v3 panel model** — not just a self-contained tile calling its own tools. A cell whose view is
`ext:<id>/<widget>` must be able to carry the same `sources[]` binding as any built-in view and
receive the **resolved data**, whatever the source kind: a **datasource** (`federation.query`),
a **flow** node port (`flows.node_state`), a **series** (history `series.read` + live
`series.watch`), or **SurrealDB directly** (`store.query`). Today the tile's bridge is walled to
its manifest `scope ∩ grant` and the cell's binding is forwarded as inert config
(`ExtWidget.tsx` → `ctx.binding`) — an extension widget cannot render platform data unless its
author predicted and declared every read verb, and even then it must re-implement query
resolution the shell already owns.

## Goals

- An editor picks an extension widget as the **view** and binds `sources[]` in the same Query
  tab used by `timeseries`/`table`/… — full source-picker vocabulary: `sql` (store.query),
  `series`, `live`, `federation`, `flows`.
- The shell resolves those sources through the shipped **`viz.query`** path (interpolation,
  transformations, per-target authorization) and hands the tile **resolved frames** — the tile
  renders, it never fetches. Live sources keep streaming into the tile without a re-mount.
- Zero new capability surface for the extension: a data-bound tile needs **no** platform read
  verbs in its manifest `scope`. Its own-tool bridge (v2) keeps working unchanged.
- v2 tiles keep working untouched (additive contract, like every dashboard contract before it).

## Non-goals

- No changes to `viz.query`, the source picker vocabulary, or the panel model — this consumes
  them.
- Not the untrusted-iframe tier for extension widgets (installed tiles stay in-process; the
  install remains the trust gate — `trust.ts`).
- Not the external-data reference extensions (timescale/mqtt-bridge) — still blocked on their
  own platform fixes (`../../extensions/reference-extensions-scope.md`).
- Not Grafana panel-plugin JSON compatibility (viz Phase 4 owns import/export).

## Intent / approach

**Frames-in, not tools-out.** Extend the widget mount contract additively to **ctx v3**:

1. **Manifest opt-in.** A `[[widget]]` table gains `data = true` (default false). A data tile
   tells the builder "show me the Query tab"; a v2 tile without it behaves exactly as today.
2. **Cell shape.** An `ext:<id>/<widget>` cell may carry the same v3 `sources[]` (+
   `transformations[]`, `fieldConfig`) as a built-in view. `dashboard.save` already persists
   these fields view-agnostically — no record change.
3. **Shell resolves.** `WidgetView`/`ExtWidget` runs the cell's targets through the same
   `viz.query` client path the built-in renderers use — under the **viewer's** token,
   `caller ∩ grant`, workspace-walled, per-target deny — and passes the result as
   `ctx.data: Frame[]` (the `lb-viz` frame shape built-ins consume).
4. **Live updates without re-mount.** `mountWidget` may return `{ update?(ctx), teardown?() }`
   instead of a bare teardown function. On a data/vars/range tick the shell calls `update(ctx)`
   with fresh frames; a tile returning only a function (v2) falls back to today's re-mount-on-
   configKey behavior. Live targets (`series.watch`, flows node-state rev bumps) feed the same
   `update` path.
5. **Editor.** `VizPicker` lists data tiles alongside built-in views; choosing one enables the
   Query tab. Non-data tiles keep the palette-entry behavior shipped in
   `widget-palette-scope.md`.

**Rejected alternative:** widen the tile's bridge to include the cell's source tools
(`store.query`, `federation.query`, …) so the extension fetches for itself. Rejected because it
hands extension code direct query reach (capability creep — the manifest scope would have to
enumerate platform read verbs, and a malicious tile could issue arbitrary allowed queries, not
just the authored one), and because every extension would re-implement interpolation +
transformations that `viz.query` already owns. Frames-in keeps the extension render-only and the
authored query the only query that runs.

## How it fits the core

- **Tenancy / isolation:** unchanged — data resolves through `viz.query` under the viewer's
  workspace-scoped token; the tile sees only resolved frames for that workspace.
- **Capabilities:** the *viewer's* grant gates each source target (existing per-target deny in
  `viz.query`); the *extension's* grant is untouched — a data tile needs no new caps. Deny path:
  a target the viewer lacks returns an error frame; the tile renders it like built-ins do.
- **Placement:** either — pure frontend contract + existing gateway routes; no role branch.
- **MCP surface:** none new. Consumes `viz.query` (get/list shape) and the shipped SSE feeds
  (`series.watch`, flows state) — the live-feed shape is already satisfied; CRUD/batch N/A.
- **Data (SurrealDB):** no new tables. The cell (with `sources[]`) persists on the existing
  `dashboard:{id}` record; the manifest `data` flag projects onto the existing `Install` →
  `ExtUi` (one new bool through `ui_decl.rs`/`lb_assets::ExtUi`).
- **Bus (Zenoh):** unchanged — live samples ride the shipped series/bus SSE.
- **Stateless extensions:** holds — the tile keeps no durable state; unmount/uninstall tears
  down `update` plumbing with the mount, as today.
- **No mocks:** tested against the real gateway (`pnpm test:gateway`) with real seeded records
  and the real `proof-panel` remote; no `*.fake.ts`.
- **SDK/WIT impact — flag loudly:** this touches the **frozen widget mount contract**
  (`app/contract.ts` mirrored in `federationWidget.ts`). It is strictly additive (ctx `v:3`,
  optional `data`, optional object return), but both mirrors + the ext-sdk template must move
  together, and `lb_assets::ExtUi`/manifest schema gain the `data` field. Version-gate on
  `ctx.v`.
- **Skill doc:** N/A — no new agent-drivable verbs; authoring an `ext:` cell with `sources[]`
  is the same `dashboard.save` surface the existing dashboard docs cover.

## Example flow

1. `proof-panel` v2 manifest adds `[[widget]] label = "Proof Chart" … data = true`; its
   `mountWidget` returns `{ update }` and renders `ctx.data` frames.
2. An editor adds a widget, picks **Proof Chart** as the view, and in the Query tab binds
   target A = SurrealDB `store.query` (builder SQL) and target B = a flow node port.
3. `dashboard.save` persists the cell `{ view:"ext:proof-panel/proof-chart", sources:[A,B] }`.
4. A viewer opens the dashboard. `ExtWidget` resolves A+B via `viz.query` under the viewer's
   token, mounts the tile with `ctx = { v:3, data:[frameA, frameB], … }`.
5. The flow's node-state rev bumps → the shell re-resolves B and calls `update(ctx)`; the tile
   re-renders in place. A viewer lacking `mcp:flows.node_state:call` gets an error frame for B
   only.

## Testing plan

Per `scope/testing/testing-scope.md` — real store/bus/gateway throughout:

- **Capability deny (mandatory):** viewer without the source tool's cap → error frame to the
  tile, other targets unaffected; tile's own bridge still denies out-of-scope `call`s.
- **Workspace isolation (mandatory):** a cell bound to workspace-A data rendered by a
  workspace-B member resolves nothing across the wall (existing `viz.query` wall, re-asserted
  through this path).
- **Contract compat:** a v2 tile (function return, no `data`) under the v3 shell — identical
  behavior, re-mount on config change.
- **Live path:** seeded series + `series.watch` target → `update` fires with fresh frames, and
  the stream tears down on unmount (hot-reload/uninstall safety).
- **E2E (`pnpm test:gateway`):** proof-panel data tile bound to a real seeded `store.query` +
  flow port, frames arrive, deny case shown red→green.

## Risks & hard problems

- **Contract drift across three mirrors** (`app/contract.ts`, `federationWidget.ts`, the
  ext-sdk template) — the ui-federation history shows these drift; move them in one slice.
- **Update-vs-remount lifecycle:** the ExtWidget effect was hard-won (StrictMode double-mount,
  orphaned roots — see the header comment). Adding an `update` path must not reintroduce the
  orphan bug; keep the per-run slot pattern.
- **Frame-shape freeze:** `ctx.data` exposes the `lb-viz` frame to third parties — it becomes a
  public contract. Version it with `ctx.v`.
- **Live fan-out cost:** N live targets × M ext cells each holding an SSE — reuse the shared
  stream plumbing built-ins use, don't open per-tile duplicates.

**Prior art — thecrew.** The graphics-canvas extension already binds scene props to *series*
through `@nube/source-picker` over its own bridge (`thecrew/ui/src/bridge/source-loaders.ts`) —
the tools-out model, correctly capability-scoped: only the picker groups its manifest grants
appear (series + assets; flows/datasources/SQL intentionally absent). That is exactly the
ceiling this scope removes: to bind more kinds, thecrew would have to request platform read
verbs it shouldn't hold. Frames-in gives its Scene tile (and any tile) the full source
vocabulary with zero added grants.

**Cleanup rider (shell↔extension leakage).** The shell's editor hardcodes thecrew's `scene:`
doc-prefix convention (`ui/src/features/dashboard/editor/tabs/useSceneDocs.ts`, consumed by
`QueryTab.tsx`) — the shell knowing one extension's convention. When this scope lands, replace
it with a manifest-declared widget option schema (e.g. `options.sceneId = {kind:"doc",
prefix:"scene:"}`) rendered generically, and delete the hook.

## Open questions

- Does `transformations[]` run before frames reach the tile (recommended: yes — same as
  built-ins, keeps parity) or does a tile ever need raw targets?
- `data = true` as a bool, or `data = "frames"` enum to leave room for a future raw/streaming
  mode?
- Should the Query tab be hideable per tile (a tile that wants exactly one series, not the full
  multi-target editor)? Recommend: no for v1 — full parity, tiles ignore extra targets.

## Related

- `widget-builder-scope.md` (the v2 contract this extends), `widget-palette-scope.md`
  (discovery), `viz/README.md` + `viz/datasource-binding-scope.md` (the panel model +
  `viz.query` this consumes), `source-picker-package-scope.md` (the picker vocabulary),
  `../../flows/flow-dashboard-binding-ux-scope.md` (flow-aware picking, composes with this),
  `../../extensions/ui-federation-scope.md` + `../../extensions/ext-sdk-scope.md` (the mount
  contract + SDK template), `../../genui/genui-scope.md` (the other "view fed by v3 sources[]"
  precedent — genui proved frames-in works).
- Code: `ui/src/features/dashboard/builder/{ExtWidget.tsx,widgetBridge.ts,federationWidget.ts}`,
  `ui/src/features/dashboard/views/WidgetView.tsx`, `rust/crates/host/src/viz/`,
  `rust/crates/host/src/ui_decl.rs`, `rust/extensions/proof-panel/ui/src/`.
