// THE one panel-data hook (viz README phasing, invariant A: "keep panel data fetch behind one data
// hook so swapping its body to the backend `viz.query` verb in Phase 3 is a one-file change"). Every
// renderer and the editor preview read panel rows ONLY through this hook — never a scattered direct
// `bridge.call`. That keeps the Phase-3 swap confined to this one file (+ its `useVizQuery` data path).
//
// PHASE 3 BODY: a NON-watch panel resolves through the backend `viz.query` verb — the WHOLE cell goes
// over as `panel` and the backend runs the transform pipeline (`lb-viz`), returning `{ frames, rows }`.
// We map `rows` → the SAME `SourceState` shape, so callers (renderer + preview) are UNCHANGED. There is
// STILL no client-side transform execution (invariant B) — the pipeline runs server-side now.
//
// LIVE-PATH EXCEPTION (honest): a `series.watch`/`bus.watch` PRIMARY target KEEPS the existing live
// `useSource` SSE path. The streaming counterpart `viz.stream` is the named Phase-3 follow-up and is NOT
// built yet, so a watch panel does not go through viz.query. We branch on whether the primary target's
// tool is a watch verb. One responsibility: give a panel its rows + state.

import type { Cell } from "@/lib/dashboard";
import { cellPrimaryTarget } from "@/lib/dashboard";
import type { VarScope } from "@/lib/vars";
import { emptyScope } from "@/lib/vars";
import { useSource, type SourceState } from "./useSource";
import { useVizQuery } from "./useVizQuery";
import { cellTools } from "../views/WidgetView";

/** The streaming verbs that keep the live `useSource` SSE path (viz.stream is the un-built follow-up). */
const WATCH_VERBS = new Set(["series.watch", "bus.watch"]);

/** Resolve a panel's data. Phase 3: a non-watch panel resolves via a debounced `viz.query(panel)` call
 *  returning `{ frames, rows }` — the SAME `SourceState` shape, so callers are unchanged. A watch primary
 *  target stays on the live `useSource` path. The `panel` is the whole cell so the backend has everything
 *  (`sources[]`/`transformations[]`/`fieldConfig`) without a signature change. */
export function usePanelData(panel: Cell, scope: VarScope = emptyScope(), refreshKey = 0): SourceState {
  const target = cellPrimaryTarget(panel);
  const isWatch = !!target && WATCH_VERBS.has(target.tool);

  // Both hooks are ALWAYS called (rules-of-hooks) — only one is "live" per panel. The live path runs the
  // watch source; the viz.query path runs the non-watch panel. The unused one resolves to a cheap state.
  const watchSource = isWatch && target ? { tool: target.tool, args: target.args } : undefined;
  const tools = cellTools(panel);
  const live = useSource(watchSource, tools, scope, refreshKey);
  const queried = useVizQuery(isWatch ? EMPTY_PANEL : panel, scope, refreshKey);

  return isWatch ? live : queried;
}

/** A no-target placeholder panel so the inactive `useVizQuery` makes no real call when a watch primary
 *  target owns the panel (keeps both hooks unconditionally mounted without a wasted gateway round-trip;
 *  the bridge rejects a no-source viz.query cheaply / the backend returns empty). */
const EMPTY_PANEL: Cell = {
  i: "__live__",
  x: 0,
  y: 0,
  w: 0,
  h: 0,
  widget_type: "chart",
  binding: { series: "" },
};
