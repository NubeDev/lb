# Scope — the shared X/Y plot builder

Status: **SHIPPED (2026-07-01)**. Durable facts promoted to
[`public/frontend/dashboard.md`](../../../../public/frontend/dashboard.md) → "X/Y plot builder".

## The ask

The built-in charts drew with hidden axes and collapsed every row to one number — no X/Y, no
multi-series, unreadable. And a user could not decide *how* to plot a result: run a query, see the
fields, pick which column is X and which are Y, choose the chart type. Make it look 10×, keep it all
Recharts, and persist the choice.

## The model (one, shared by both surfaces)

`ui/src/lib/charts/` — pure + tested:

- **`PlotSpec`** `{ type: line|area|bar|scatter|pie|histogram, xField, yFields[], seriesField?,
  stacked?, bins?, horizontal?, smooth? }` — the canonical "how to draw these rows". Persisted verbatim
  by both surfaces.
- **`inferFields(rows)`** → `time | number | category` per column, by sampled values (no schema).
- **`buildPlot(rows, spec)`** → the Recharts frame: multi-series wide frames, long→wide `seriesField`
  pivot, pie aggregate, histogram bin. The three shape changes `rowNumber` could not express.
- **`suggestPlot(rows)`** — a TS twin of the host `pick_chart`, so a dashboard panel and a channel
  result open on the same default; the builder is only for *changing* it.

## The renderer + builder

`ui/src/features/charts/` — `PlotChart` (real titled X/Y axes, ticks, dashed gridlines,
`ResponsiveContainer`, themed tooltip, legend, reduced-motion-aware draw-in, empty/table-only states)
and `PlotBuilder` (chart-type toggle + X/Y/series field pickers + per-type modifiers + live preview).
Styling is `chartTheme` (off the HSL tokens). Both surfaces render the SAME component — no drift.

## Persistence

- **Dashboard**: `Cell.options.plot` (a `PlotSpec`), saved through `dashboard.save`
  (`OWNED_OPTION_KEYS` gains `plot`; round-trip test unchanged). A new **Plot** tab in the panel editor
  runs the draft query live and mounts the builder. No backend change.
- **Channel**: a `query_result` item is authored by the query worker and immutable, so the viewer's
  choice is **separate per-user state**: new host-native verbs `channel.chart_pref.get`/`.set`
  (record `channel_chart_pref:[ws, cid__item__user]`). Gated by the channel `sub` cap (read the channel
  ⇒ save how you plot it) behind the `mcp:channel.chart_pref.<verb>:call` grant (member-level). Two
  viewers can plot the same result differently; the canonical result never changes.

## Invariants held

- README §3: symmetric nodes / one datastore / state-vs-motion (the pref is state; nothing on the bus)
  / capability-first (both gates, opaque) / workspace is the hard wall (per-ws record, tested).
- FILE-LAYOUT: one responsibility per file; one model, one renderer, one builder reused, not copied.
- Rule 9 (no mocks): backend proven against a real booted `Node`; the model proven with real rows.

## Follow-ups

- `pnpm test:gateway` round-trip for `chartPref.api` against a real spawned node.
- Host tool **descriptors** for the two verbs (so they list in `tools.catalog`).
- Opt-in `plot` path for the `scatter`/`histogram` dashboard views + Grafana-JSON import/export mapping.
