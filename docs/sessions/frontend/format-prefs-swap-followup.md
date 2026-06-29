# Follow-up — swap `fieldconfig/format.ts` to the real `format.*` MCP call (DEFERRED, its own session)

Status: **deferred** (named, not silent). Surfaced during dashboard-viz Phase 3
([`dashboard-viz-phase3-session.md`](dashboard-viz-phase3-session.md)), where the brief invited the
contained swap "while you're here" now that `lb-prefs` has shipped.

## The ask

`ui/src/features/dashboard/fieldconfig/format.ts` is THE one viz formatter (canonical value + `fieldConfig`
unit/decimals → display string). Phase 1 shipped it as a **fallback** (canonical number + static unit
label + local round) behind a `format.*`-shaped call site, with a `viaPrefs` flag as the documented
swap-point guardrail. `lb-prefs` has since shipped the real grant-free `format.quantity`/`format.number`/
`format.datetime` MCP verbs. The follow-up: replace `fallbackFormat` with the real `format.*` dispatch
behind the same `formatValue` signature, setting `viaPrefs: true` — no `fieldConfig` schema change, no
re-save.

## Why it is deferred (not a contained swap)

`formatValue` is **synchronous** and is called **during render** at ~13 callsites across 8 renderers
(stat/gauge/bargauge/table/barchart/piechart value cells, the timeseries axis/legend, the fieldConfig
preview). The real `format.*` verbs are an **async MCP call** (a bridge round-trip). Swapping
`fallbackFormat` for the async call would force `formatValue` async, which cascades into making every one
of those render callsites async — well beyond "a contained swap behind the same signature." Per the
HOW-TO-CODE rule "if it turns out to be more than a contained swap, make it its own session and say so" —
this is that case. `format.ts` is therefore left as-is (honest fallback, `viaPrefs: false`).

## The contained approach for the dedicated session (the lead, recorded)

Two viable shapes that keep the renderers synchronous (so the swap stays small):

1. **Format-on-fetch (preferred).** Resolve the displayed values' formatted strings in `viz.query`/
   `useVizQuery` — once, async, off the render path — and attach them alongside `rows` (a parallel
   `formatted` map keyed by field+value, or per-cell). Renderers read the prefetched string synchronously.
   This fits the Phase-3 doctrine (presentation resolved near the data fetch) and keeps `format.*` the one
   formatter. Note: data shape stays canonical (`viz.query` returns canonical frames); the formatted
   strings are an *additional* presentation layer, not a replacement of the canonical values.
2. **A prefetched sync format cache.** A small `usePrefsFormat` hook resolves the viewer's prefs + a memoized
   `format.*` result map for the panel's value set, exposing a synchronous `format(value, opts)` the
   renderers call. More moving parts than (1) but keeps `format.ts` the chokepoint.

Either way the `viaPrefs` flag flips to `true` at the swap point, and the existing fallback stays as the
honest degradation when prefs/format is unreachable.
