# Frontend dashboard — surface extension `[[widget]]` tiles in the builder palette (session)

- Date: 2026-06-28
- Scope: ../../scope/frontend/dashboard/widget-palette-scope.md
- Stage: post-S8, building on the shipped widget-builder v2 (STATUS.md "Slices in flight")
- Status: done
- Public: ../../public/frontend/dashboard.md
- Tests: ui/src/features/dashboard/builder/widgetBuilder.test.ts (unit),
  ui/src/features/dashboard/builder/widgetBuilder.gateway.test.tsx (real gateway),
  ui/src/features/dashboard/DashboardView.gateway.test.tsx (routing-context gate)

## Goal

Close the one discovery gap in the shipped widget-builder v2: a dashboard **editor** gets a NEW
palette option per installed extension's packaged `[[widget]]` tile (e.g. `proof-panel`'s **Proof
Ping**), gated to users holding `mcp:dashboard.save:call`. No backend change, no v2 contract change.

Acceptance (the scope's testing plan): `extWidgetEntries` emits one "widget" entry per tile;
real-gateway round-trip (install → palette lists "Proof Ping" → select → preview the real
`proof.demo` latest → Add persists `view:"ext:proof-panel/proof-ping"` → reload re-renders);
capability deny (no add affordance + server denies `dashboard.save`); workspace isolation; trust-tier
routing from the palette path.

## What changed

Three FILE-LAYOUT-respecting changes (one responsibility per file):

1. `ui/src/features/dashboard/builder/ExtWidget.tsx` — `export` the existing `widgetIdOf` helper so the
   picker derives the cell key with the SAME logic the renderer uses (scope: "match it, don't reinvent").
2. `ui/src/features/dashboard/builder/sourcePicker.ts` — add `"widget"` to `SourceEntry["group"]`; add
   `extWidgetEntries(rows)` (one entry per `row.widgets[]` tile, group `"widget"`, label
   `${row.ext} · ${tile.label}`, carries `icon` + the resolved `ext:<id>/<widget>` view key); fold into
   `buildSourceEntries`. Existing tool-harvesting (`extension`/`action`) unchanged.
3. `ui/src/features/dashboard/builder/WidgetBuilder.tsx` — add a `PickerGroup group="widget"
   label="Extension widgets"`; when a widget entry is selected, force the candidate cell's
   `view`/`widget_type` to the `ext:<id>/<widget>` key and hide the view chooser (`viewsFor` returns the
   single packaged view); preview routes through the shipped `WidgetView` → `ExtWidget` over the real
   bridge. Added a `canEdit` prop — the whole "Add widget" surface renders only when `canEdit`.
4. `ui/src/features/dashboard/DashboardView.tsx` — derive `canEdit = hasCap(ctx.caps, CAP.dashboardSave)`
   from the routing context the shell already holds (the same source the nav uses) and thread it into
   `WidgetBuilder`. No new backend read.

## Decisions & alternatives

- **A new `widget` group, not reuse `extension`.** A tile is a finished widget; the `extension` group
  offers an extension's raw tools. Different author intent, different cell shape — distinct entries (per
  scope).
- **Cap source = routing context.** `useAppRoutingContext().caps` + `hasCap(caps, CAP.dashboardSave)`,
  threaded `DashboardView → WidgetBuilder` as `canEdit`. Rejected: a fresh backend read (the shell already
  holds the grant); rejected: server-only gate (poor affordance — show nothing a viewer can't complete).
- **`widgetIdOf` exported from `ExtWidget`, not duplicated.** The cell key the palette builds must equal
  the key the renderer parses; sharing the function guarantees it.

## Tests

All against real infra (real spawned gateway, real installed proof-panel via the `/_seed` real write
path, real `dashboard.save`/`ext.list`/`series.latest`); no mocks, no `*.fake.ts`. Mandatory categories:

- **Capability deny (headline):** `edit-cap gate` — a viewer with `canEdit=false` renders an EMPTY add
  surface (no source picker, no add button); a direct `dashboard.save` from a principal lacking
  `mcp:dashboard.save:call` is **denied server-side** (the host backstop, UI gate bypassed).
- **Workspace isolation:** a ws-B editor's `extWidgetEntries(ext.list)` lists only ws-B's tile
  (`mqtt-bridge · Cooler Switch`), never ws-A's `Proof Ping` — the hard wall on the palette read.
- **Builder round-trip (real gateway):** install proof-panel `[[widget]]` → the "Extension widgets" group
  lists `proof-panel · Proof Ping` → select → view chooser disappears → preview mounts the real
  `ExtWidget` (`[data-ext-widget="proof-panel"]`) over the real bridge (the tile's `proof.demo` latest =
  seeded 21, asserted over the live bridge) → **Add** emits a `v:2, view:"ext:proof-panel/proof-ping"`
  cell with no source → persisted via real `dashboard.save` → `getDashboard` re-reads it.
- **Trust-tier from the palette path:** proof-panel (non-allow-listed key) added from the palette renders
  **sandboxed** (`data-tier="iframe"`), never in-process — the tier is the tile's, decided by key.
- **Unit (extWidgetEntries):** one `widget` entry per `[[widget]]` tile, label `<ext> · <tile.label>`,
  `viewKey` = the renderer's `widgetIdOf` slug, no source/action; folded into `buildSourceEntries`
  alongside the tool entries; disabled extension contributes none.

- **Trust-tier routing (updated):** an INSTALLED extension widget federates **in-process** (the install
  is the trust gate); a scripted view stays sandboxed. Asserted in the unit `trust-tier routing` case and
  re-asserted from the palette path in the two gateway trust-tier tests + the round-trip
  (`[data-tier="in-process"]`, no `[data-widget-iframe]`).

Green output (after the trust-tier fix):

```
$ pnpm test
 Test Files  9 passed (9)
      Tests  48 passed (48)
   ✓ src/features/dashboard/builder/widgetBuilder.test.ts (13 tests)

$ pnpm test:gateway
 Test Files  25 passed (25)
      Tests  110 passed (110)
   ✓ src/features/dashboard/builder/widgetBuilder.gateway.test.tsx (17 tests)
   ✓ src/features/dashboard/DashboardView.gateway.test.tsx (3 tests)

$ npx playwright test e2e/dashboard-widget.spec.ts   → 1 passed   (in-process tile, real value)
$ npx playwright test e2e/proof-panel.spec.ts        → 1 passed   (page e2e, no regression)
```

## Follow-up (same session): the iframe tier couldn't render an installed widget

Publishing `proof-panel` live and adding its tile surfaced a real shipped-renderer bug: the tile
rendered **blank** with `Failed to resolve module specifier "react"`. Root cause — `proof-panel`'s
remote **externalizes React** (the rubix import-map pattern) to be resolved by the **shell import map**,
which only exists **in-process**; but `ExtWidget` routed any non-allow-listed key to the **iframe** tier,
whose opaque-origin sandbox has no import map (and whose `template` engine never executes a federated
remote anyway). So an installed extension widget could never render in the iframe tier.

**Decision (best long-term, per the user):** drop the iframe tier for extension widgets entirely.
**Installing an extension already passes the publish/install capability gate** — that *is* the trust
decision — so an installed widget federates **in-process** (the only tier its bundle is built for). The
sandboxed iframe stays for **scripted author code** (`plot`/`d3`/`template`) typed into a cell, which is
the genuine untrusted-code case. Rejected: teaching the iframe runtime to load ext remotes (esm.sh import
map + relaxed CSP) — it forces a second React copy and adds surface for a tier no installed widget needs.

Changes: `trust.ts` `extWidgetTier()` → always `"in-process"` (scripted stays `"iframe"`); `ExtWidget`
dropped its dead iframe branch + `remoteIframeCode` and always federates via `loadRemoteWidgetMount`.
Full entry: [`../../debugging/frontend/ext-widget-iframe-tier-cannot-resolve-bare-react.md`](../../debugging/frontend/ext-widget-iframe-tier-cannot-resolve-bare-react.md).

**Live e2e (the proof a unit test can't give — the failure only shows in a real browser):** new
[`ui/e2e/dashboard-widget.spec.ts`](../../../ui/e2e/dashboard-widget.spec.ts) — built shell on :4173 +
real node on :8080: login → Dashboards → create → pick "proof-panel · Proof Ping" from the **Extension
widgets** group → the tile mounts **in-process** (`[data-ext-widget][data-tier="in-process"]`) with the
host's single React → renders the **real `proof.demo` value over the bridge** → Add persists → re-renders
in the grid; **no iframe, no error wrapper, no "Failed to resolve module specifier react", no hook-call
crash**. The existing `proof-panel.spec.ts` page e2e still passes (no regression). Both green:

```
$ npx playwright test e2e/dashboard-widget.spec.ts   → 1 passed
$ npx playwright test e2e/proof-panel.spec.ts        → 1 passed
```

Live publish confirmed: `make publish-ext EXT=proof-panel` → HTTP 204 (installed + loaded); `ext.list`
shows the `Proof Ping` widget; `remoteEntry.js` served HTTP 200.

## Debugging

[`debugging/frontend/ext-widget-iframe-tier-cannot-resolve-bare-react.md`](../../debugging/frontend/ext-widget-iframe-tier-cannot-resolve-bare-react.md)
— resolved, with the regression in the unit `trust-tier routing` case + the two gateway trust-tier tests
+ the live `dashboard-widget.spec.ts`.

One in-session correction worth recording (not a debug entry — caught by `tsc` before any commit): the
scope said "set `widget_type` to the same key", but `Cell.widget_type` is the v1 `WidgetType` union
(`chart|stat|gauge`), not the v2 `view`. `cellView` reads `view` first, so the packaged tile renders from
`view: "ext:<id>/<widget>"` and `widget_type` stays the harmless `"chart"` every other v2 cell carries.
The scope's intent (the cell points at the packaged key) is met via `view`; recorded in the scope's open
questions.

## Decisions & alternatives

(see "Decisions & alternatives" above — unchanged)

## Public / scope updates

- Promoted to [`public/frontend/dashboard.md`](../../public/frontend/dashboard.md): the "Extension widgets"
  palette group + the edit-cap gate are now shipped truth.
- [`scope/frontend/dashboard/widget-palette-scope.md`](../../scope/frontend/dashboard/widget-palette-scope.md)
  flipped to **SHIPPED**; its open questions were all pre-decided — added the `widget_type`/`view` note.
- `STATUS.md` "Slices in flight" updated.

## Follow-up (same session): a 2nd widget that uses the SSE (live tile)

To exercise the **live feed** end to end and to prove the picker's "one entry per `[[widget]]`" with a
real N>1 extension, `proof-panel` now ships a **second** packaged tile, **Proof Ping Live**. Where the
first tile reads `proof.demo` once (`bridge.call("series.latest")`), the live tile **subscribes** to its
motion: `bridge.watch("series.watch", {series:"proof.demo"})` → the shipped `openSeriesStream` → the
gateway SSE `GET /series/proof.demo/stream` → the workspace motion subject. It backfills with
`series.latest` (non-empty before the first sample), then updates per live sample with no reload/poll,
flipping a "live" badge on. The whole SSE plumbing was already shipped (bridge `watch`, the SSE endpoint,
`openSeriesStream`) — the only gap was that no widget *used* it; this adds the worked example.

Extension changes (NOT the platform/shell): new
[`WidgetLiveTile.tsx`](../../../rust/extensions/proof-panel/ui/src/app/WidgetLiveTile.tsx);
`mountWidget` now **dispatches by `widgetId`** (`proof-ping` → static, `proof-ping-live` → live) — proving
the renderer's `widgetIdOf` slug is the contract between the manifest label and the cell key; a second
`[[widget]]` block in [`extension.toml`](../../../rust/extensions/proof-panel/extension.toml) with
`scope = [..., "series.watch"]`, and `mcp:series.watch:call` added to `[capabilities] request` so
`ui_decl::narrow` keeps `series.watch` in the tile's granted scope (verified live in `ext.list`).

Tests: **+3 proof-panel unit** ([`WidgetLiveTile.test.tsx`](../../../rust/extensions/proof-panel/ui/src/app/WidgetLiveTile.test.tsx):
backfill → live-tick ×2 → "live" badge; unsubscribe-on-unmount = stateless eviction; deny → "no access")
over a `watchBridge` test double (the bridge interface, not a fake node — testing-scope §0). **+1 live
Playwright e2e** [`ui/e2e/dashboard-widget-live.spec.ts`](../../../ui/e2e/dashboard-widget-live.spec.ts):
built shell + real node — add **Proof Ping Live** from the palette → tile mounts in-process → backfills →
**write a new `proof.demo` sample over real ingest → the tile ticks to it (then again) with NO reload**,
"live" badge on. Picker now offers two entries (`proof-panel · Proof Ping` and `· Proof Ping Live`); the
static e2e was made exact-label to disambiguate. Green:

```
$ proof-panel/ui vitest run         → 15 passed (WidgetLiveTile.test.tsx: 3)
$ npx playwright test e2e/dashboard-widget-live.spec.ts  → 1 passed  (live tick, no reload)
$ npx playwright test (all 3 dashboard/page e2e)         → 3 passed
$ pnpm test (shell)                 → 48 passed
$ pnpm test:gateway (shell)         → 110 passed
```

Live publish confirmed: `make publish-ext EXT=proof-panel` → HTTP 204; `ext.list` shows BOTH widgets,
the live one carrying `series.watch` in scope.

**Answering "do we have all the plumbing for extension widgets + dashboard?":** yes — the SSE chain
(bridge `watch` → `openSeriesStream` → `GET /series/{s}/stream` → motion subject) was fully shipped; this
slice adds the first widget that *uses* it. One nuance recorded for future ext authors: a live tile must
name `series.watch` in its `[[widget]].scope` AND have `mcp:series.watch:call` in the capability request
(else `ui_decl::narrow` drops it); the SSE endpoint itself authorizes on `series.read`.

## Live verification

Not run as a manual `make publish-ext` + browser pass this session — no node was running and the
"open the builder, confirm Proof Ping is addable" step needs a browser not available headlessly here.
The equivalent path is covered automatically and authoritatively by the real-gateway round-trip test
(real spawned gateway + a real installed `proof-panel` `[[widget]]`: palette lists "Proof Ping" → select
→ preview mounts the real `ExtWidget` → Add persists the cell → reload re-reads it). To verify live:
`make cloud` (or `make dev`) → `make publish-ext EXT=proof-panel` → open the dashboard builder as an
editor → confirm "proof-panel · Proof Ping" appears under "Extension widgets" and adds.

## Follow-ups

- Deferred (named in scope, not this slice): a disabled/ghosted tile shown to read-only viewers as "ask an
  editor to add" — this slice simply hides the add surface.
- The DashboardView gateway test now wraps the view in a real `RoutingContextProvider` fed the real
  session's caps (the live shell's source) — no mock; a future deny-variant can pass fewer caps to drive
  the hidden-builder case through the full view.
- STATUS.md updated? **Yes.**
