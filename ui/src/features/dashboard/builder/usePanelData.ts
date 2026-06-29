// THE one panel-data hook (viz README phasing, invariant A: "keep panel data fetch behind one data
// hook so swapping its body to the backend `viz.query` verb in Phase 3 is a one-file change"). Every
// renderer and the editor preview read panel rows ONLY through this hook — never a scattered direct
// `bridge.call`. That keeps the Phase-3 swap (client fetch → `viz.query(panel) -> { frames }` with the
// `lb-viz` transform pipeline applied server-side) confined to this one file.
//
// PHASE 1 BODY: a no-transform panel resolves its PRIMARY target through the shipped v2 bridge (the
// existing `useSource`). There is NO client-side transform execution (invariant B) — `transformations[]`
// is config on the cell that the backend will run in Phase 3; Phase 1 ignores it for data (and the
// editor's Transform tab only edits that config). One responsibility: give a panel its rows + state.

import type { Cell } from "@/lib/dashboard";
import { cellPrimaryTarget } from "@/lib/dashboard";
import type { VarScope } from "@/lib/vars";
import { emptyScope } from "@/lib/vars";
import { useSource, type SourceState } from "./useSource";
import { cellTools } from "../views/WidgetView";

/** Resolve a panel's data. Phase 1: the primary target over the v2 bridge, no transforms. Phase 3:
 *  this body becomes a debounced `viz.query(panel)` call returning canonical frames — the SAME shape,
 *  so callers (renderer + preview) are unchanged. The `panel` is the whole cell so the Phase-3 body has
 *  everything (`sources[]`/`transformations[]`/`fieldConfig`) without a signature change. */
export function usePanelData(panel: Cell, scope: VarScope = emptyScope(), refreshKey = 0): SourceState {
  // The primary target generalizes the v2 `source` (a v2 cell reads as `sources[0]` via `cellSources`).
  const target = cellPrimaryTarget(panel);
  const source = target ? { tool: target.tool, args: target.args } : undefined;
  // The cell's tool set (host ∩ grant leash) — the same set the bridge forwards, derived once.
  const tools = cellTools(panel);
  return useSource(source, tools, scope, refreshKey);
}
