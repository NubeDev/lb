# Session — the shared X/Y plot builder (dashboard + channel query charts)

**Date:** 2026-07-01
**Ask (user):** "10× the dashboard graphs — they look like shit, no x/y. All charts must be Recharts.
Save the options in the backend if needed. Let the user run a query, see the fields, and pick how to
plot x/y via a nice UX. Amazing UX/UI, take extra care." (via `/impeccable`.)

## What was wrong

Everything was **already Recharts** — the problem was styling + a missing data model:

- **Dashboard panels** (`views/timeseries`, `views/barchart`, `widgets/recharts.tsx`) drew charts with
  `<XAxis hide/> <YAxis hide/>` and collapsed every row to a single number via `rowNumber`
  (`.value ?? .payload`). No real X field, no multi-series — so no axes, no legend, nothing to read.
- **Channel query charts** (`channel/query/ChartView.tsx`) showed axes but the **host** picked the
  chart type + x/series (`pick_chart`), so a viewer could not choose how to plot.
- Two hand-drawn Recharts code paths that drifted.

## What shipped

One model, one renderer, one builder — reused by **both** surfaces (FILE-LAYOUT rule #8):

- **`ui/src/lib/charts/`** — the shared model (pure, tested): `PlotSpec` (`type`/`xField`/`yFields`/
  `seriesField`/`stacked`/`bins`/…), `inferFields` (types columns time/number/category by sampled
  values), `buildPlot` (rows+spec → Recharts frame: multi-series, long→wide pivot, pie aggregate,
  histogram bin), `suggestPlot` (a TS twin of the host `pick_chart` so both surfaces open on the same
  default). **16 unit tests.**
- **`ui/src/features/charts/`** — the 10× renderer + builder: `PlotChart` (real **titled** X/Y axes,
  ticks, dashed gridlines, `ResponsiveContainer`, themed rich tooltip, legend, reduced-motion-aware
  draw-in, and honest empty/table-only states), `PlotBuilder` (chart-type toggle, X/Y/series pickers as
  typed field pills, per-type modifiers, **live preview**), `chartTheme` (axis/grid/tooltip + a
  categorical palette off the HSL tokens).
- **Channel** — `ChartView` now renders a `PlotSpec` through the shared `PlotChart`; `QueryCard` gained
  Chart/Table/**Customize** modes. Customize opens the builder; a table-only result can be plotted from
  scratch. The choice **persists per-viewer** and is merged over the host default at load.
- **Dashboard** — `timeseries`/`barchart`/`piechart` panels render through `PlotChart` when a `plot`
  spec is configured (additive: no spec keeps the legacy chart byte-for-byte). A new **Plot** tab in the
  panel editor runs the draft query live (`usePanelData`), types the fields, and mounts the same
  `PlotBuilder`. Persisted in `Cell.options.plot` via `dashboard.save` (registered in
  `OWNED_OPTION_KEYS`) — **no backend change**.

## Backend — "best long term" channel persistence

A `query_result` item is authored by `system:query-worker` and is immutable; `channel.edit` requires
author ownership. So a viewer's plot choice is **separate per-user state**, not an edit of the result:

- New host-native MCP verbs **`channel.chart_pref.get` / `.set`** (`crates/host/src/channel/chart_pref.rs`
  + `chart_pref_tool.rs`). Record `channel_chart_pref:[ws, cid__item__user]`, workspace = namespace
  (hard wall), keyed per-user (two viewers can plot the same result differently).
- Auth: the outer dispatch runs `mcp:channel.chart_pref.<verb>:call` (added to gateway `member_caps()`);
  the verb re-checks the channel **`sub`** gate (`bus:chan/{cid}:sub`) — read the channel ⇒ save how you
  plot it. Denials opaque.
- Dispatch: narrow `channel.chart_pref.` prefix added to `is_host_native` + the dispatch chain in
  `tool_call.rs` (NOT all `channel.*`, so future channel extension tools still route to the registry).
- Reached from the UI via the universal `mcp_call` bridge (`lib/channel/chartPref.api.ts`) — no bespoke
  gateway route.

## Tests (green)

- UI unit: `pnpm test` → **258 passed** (incl. the 16 new `lib/charts` tests; `cellEditorState`
  round-trip still holds with the new `plot` key).
- Rust: `cargo test -p lb-host --test channel_chart_pref_test` → **3 passed** — set→get round-trip +
  **per-user** isolation, **capability-deny** (both the MCP grant gate and the channel `sub` gate,
  opaque), and **workspace isolation** (ws-B never reads a ws-A pref). Existing `dashboard_test` /
  `channel_query_worker_test` unchanged. `cargo build -p lb-host -p lb-role-gateway` clean; `cargo fmt`.

## Notes / follow-ups

- Recharts v3 types the custom-tooltip payload loosely — `ChartTooltip` narrows its own props rather
  than extending `TooltipProps` (a build fix, not a runtime bug; no debug entry needed).
- Follow-ups: a `pnpm test:gateway` round-trip for `chartPref.api` against a real spawned node;
  optional host **tool descriptors** for the two verbs so they appear in `tools.catalog`; extend the
  opt-in `plot` path to `scatter`/`histogram` dashboard views.
