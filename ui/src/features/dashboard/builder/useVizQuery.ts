// The Phase-3 viz.query fetch, now on the dashboard READ CACHE (dashboard-query-cache-scope). It calls the
// backend host verb `viz.query` through the SAME WidgetBridge a cell rides (`mcp:viz.query:call`), passing
// the WHOLE cell as `panel` and the resolved VarScope as `scope`; the backend runs the transform pipeline
// (`lb-viz`) and returns `{ frames, rows }`, which we map to the SAME `SourceState` shape (invariant B: NO
// client-side transform execution — callers unchanged).
//
// WHAT CHANGED (cache): the fetch is a react-query `useQuery` keyed on the CANONICAL RESOLVED SPEC —
// `{sources, transformations, fieldConfig, source, scope, tick}` — NOT the whole-panel JSON. So the three
// editor consumers (probe/preview/plot) that mount `useVizQuery` for the same draft now share ONE cache
// entry → ONE gateway round-trip, and a title/layout/option edit no longer re-keys → no refetch (scope
// goal 2). The 200ms debounce moved to the KEY INPUT (one debounce, not one per consumer). A denied/missing
// cap → the bridge throws → an honest `denied` state, never a fabricated value (CLAUDE §9).

import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";

import type { Cell, Target } from "@/lib/dashboard";
import { cellPrimaryTarget, cellSources } from "@/lib/dashboard";
import { cellTools } from "../views/WidgetView";
import { useDashboardWs } from "../cache/useDashboardWs";
import { useDebounced } from "../cache/useDebounced";
import { vizQueryKey } from "../cache/queryKeys";
import type { VarScope } from "@/lib/vars";
import { emptyScope, interpolateArgs } from "@/lib/vars";
import { makeWidgetBridge } from "./widgetBridge";
import type { SourceState } from "./useSource";

/** The qualified host verb this hook dispatches (cap `mcp:viz.query:call`). */
export const VIZ_QUERY_TOOL = "viz.query";

const DEBOUNCE_MS = 200;

/** A canonical column frame, as `viz.query` returns it. */
interface Frame {
  refId?: string;
  name?: string;
  fields: Array<{ name: string; type?: string; values: unknown[] }>;
  length?: number;
}

/** The `viz.query` result — canonical `frames` PLUS the primary frame flattened to `rows`. */
interface VizQueryResult {
  frames?: Frame[];
  rows?: Array<Record<string, unknown>>;
}

/** Pull a scalar "latest" from the rows (mirrors `useSource.toLatest`). */
function toLatest(rows: Array<Record<string, unknown>>): unknown {
  if (!rows.length) return null;
  const last = rows[rows.length - 1];
  return last.value ?? last.payload ?? last;
}

/** Resolve a panel's rows via the backend `viz.query` verb, through the shared read cache. The cell goes
 *  over as `panel` whole; the resolved scope as `scope`. Consumers with the same resolved spec share one
 *  cache entry; the key is debounced so editor keystrokes fire one call, not one per stroke. */
export function useVizQuery(panel: Cell, scope: VarScope = emptyScope(), refreshKey = 0): SourceState {
  const ws = useDashboardWs();

  // The bridge leash = the cell's tool set ∩ grant, PLUS viz.query itself (the cell calls it). Re-create
  // the bridge only when that set changes.
  const tools = cellTools(panel);
  const toolsKey = [...tools, VIZ_QUERY_TOOL].join("|");
  // eslint-disable-next-line react-hooks/exhaustive-deps -- re-create the bridge only when the tool SET changes
  const bridge = useMemo(() => makeWidgetBridge([...tools, VIZ_QUERY_TOOL]), [toolsKey]);

  // Interpolate each target's args against the resolved scope BEFORE the call (a `${host}` repoints the
  // series exactly as the shipped path did). The panel sent over carries the interpolated `sources[]`.
  const resolvedPanel = useMemo(() => {
    const sources: Target[] = cellSources(panel).map((t) => ({
      ...t,
      args: interpolateArgs(t.args ?? {}, scope) as Record<string, unknown>,
    }));
    return { ...panel, sources };
  }, [panel, scope]);

  const hasTarget = !!cellPrimaryTarget(panel)?.tool;

  // The de-dup key = the CANONICAL resolved spec: only the fields that change what `viz.query` returns.
  // Title/layout/panel-option edits are NOT here, so they don't re-key → no refetch (scope goal 2).
  const spec = useMemo(
    () => ({
      sources: resolvedPanel.sources,
      transformations: (resolvedPanel as { transformations?: unknown }).transformations,
      fieldConfig: (resolvedPanel as { fieldConfig?: unknown }).fieldConfig,
      source: (resolvedPanel as { source?: unknown }).source,
      scope,
      tick: refreshKey,
    }),
    [resolvedPanel, scope, refreshKey],
  );

  // Debounce the KEY INPUT: a burst of keystrokes settles to one new key → one call (scope: debounce risk).
  const debouncedSpec = useDebounced(spec, DEBOUNCE_MS);

  const query = useQuery({
    queryKey: vizQueryKey(ws, debouncedSpec),
    // Nothing to query without a resolvable primary target — honest empty, no wasted round-trip.
    enabled: hasTarget,
    queryFn: () => bridge.call<VizQueryResult>(VIZ_QUERY_TOOL, { panel: resolvedPanel, scope }),
  });

  if (!hasTarget) return { rows: [], latest: null, loading: false, denied: true };
  if (query.isError) return { rows: [], latest: null, loading: false, denied: true };
  const rows = Array.isArray(query.data?.rows) ? query.data!.rows! : [];
  return { rows, latest: toLatest(rows), loading: query.isLoading, denied: false };
}
