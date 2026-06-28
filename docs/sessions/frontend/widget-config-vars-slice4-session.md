# Frontend dashboard — auto-refresh + live events — Slice 4 (session)

- Date: 2026-06-28
- Scope: ../../scope/frontend/dashboard/widget-config-vars-scope.md (Slice 4)
- Status: done
- Public: ../../public/frontend/dashboard.md → "Auto-refresh + live events"
- Tests: ui/src/features/dashboard/useAutoRefresh.test.ts (4), busBridge.gateway.test.tsx (3),
  + the transport round-trip in role/gateway/tests/bus_routes_test.rs

## Goal

A refresh-interval picker (URL `?refresh=30s`) that on each tick re-resolves query variables + re-runs
each read cell's source (poll STATE), composing with live `bus.watch`/`series.watch` (stream MOTION). Pause
when the tab is hidden; dedupe in-flight.

## What shipped

- `ui/src/features/dashboard/useAutoRefresh.ts` — `refreshMs` (parse `5s`/`1m`/…) + `useAutoRefresh` (a
  `refreshKey` that bumps every interval; pauses on `document.hidden`; clears/reschedules on interval
  change; off = frozen key).
- `ui/src/features/dashboard/RefreshControl.tsx` — the off/5s/10s/30s/1m/5m/15m dropdown, URL-synced via
  `onSearchChange({ ...range, refresh })` (the `refresh` param shipped in Slice 2's search schema).
- `refreshKey` threaded DashboardView → Grid → WidgetHost → WidgetView → read views → `useSource` (folded
  into the source key so a non-watch read re-runs) and → VariableBar (`useVariableOptions` re-resolves a
  query variable). In-flight dedupe is the re-keyed effect's job (the prior run cancels).
- **Live push wiring:** `ui/src/lib/dashboard/bus.stream.ts` (`openBusStream` — `GET /bus/stream?subject=
  &token=`, `message` events) + the WidgetBridge `watch` now routes `bus.watch` → `openBusStream` (keyed
  by `args.subject`) and `series.watch` → `openSeriesStream` (by `args.series`). Refresh polls state;
  watch streams motion; they compose, and a cell declares which it uses.

## Decisions

- **Refresh is a URL param, the tick is local.** The interval is shareable (`?refresh=30s`); `useAutoRefresh`
  owns the cadence and the visibility pause (no work for an unseen dashboard — the thundering-herd guard).
- **One key, two consumers.** `refreshKey` re-keys both `useSource` (cell reads) and `useVariableOptions`
  (query vars) — a single tick re-resolves the whole dashboard's state without a bespoke per-cell timer.
- **bus.watch ≠ series.watch transport.** A generic subject contains `/`, so it streams over
  `/bus/stream?subject=` (the platform-fix SSE), distinct from the series-keyed `/series/{s}/stream`.

## Tests + green output

Unit — `vitest run` (full): **110 passed** (`useAutoRefresh`: parse table, tick bumps the key, off never
ticks, interval change reschedules).

Real-gateway — `busBridge.gateway.test.tsx`: **3 passed** — a granted `bus.publish` through the leashed
WidgetBridge → `{ok:true}`; a publish to a tool outside the cell's set rejected at the bridge leash; a
reserved subject (`series/...`) refused server-side. The publish→watch **SSE** round-trip is proven at the
transport in `role/gateway/tests/bus_routes_test.rs` (jsdom has no `EventSource`).

## Mandatory categories

- **Auto-refresh:** the tick re-resolves variables + re-runs sources (the shared `refreshKey`); `off`
  stops; an interval change reschedules — unit-asserted.
- **Live (real gateway):** `bus.publish` → a `bus.watch` subscriber receives it over the new SSE
  (transport test); `series.watch` still works (unchanged path). Capability deny + the subject wall hold
  (the bus platform-fix tests + the bridge-leash test here).

## Follow-ups

Next: Slice 5 — the JSON payload builder (uses the shared lib + the bus.publish sink + a write tool).
