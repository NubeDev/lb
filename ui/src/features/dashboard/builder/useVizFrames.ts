// The frames-in read for a v3 ext DATA tile (ext-widget-source-binding scope). It resolves a cell's
// `sources[]` through the SAME backend `viz.query` verb, the SAME WidgetBridge leash, and — crucially —
// the SAME react-query cache key (`vizQueryKey`) as `useVizQuery`. So an ext data tile and a built-in
// renderer bound to the same spec share ONE gateway round-trip (no per-tile duplicate stream), and the
// per-target authorization + workspace wall live entirely in `viz.query` (a denied target degrades to
// an honest empty frame server-side, exactly like the built-ins). The tile RENDERS these frames — it
// never fetches, never sees a token or the DB.
//
// This is the frames counterpart to `useVizQuery` (which maps the same result down to flattened `rows`
// for the built-in stat/table/timeseries renderers). Same call, same cache entry — different projection.

import { useMemo } from "react";
import { QueryClient, useQuery } from "@tanstack/react-query";

import type { Cell, Target } from "@/lib/dashboard";
import { cellPrimaryTarget, cellSources } from "@/lib/dashboard";
import { cellTools } from "../views/WidgetView";
import { useDashboardWsOptional } from "../cache/useDashboardWs";
import { useDebounced } from "../cache/useDebounced";
import { vizQueryKey } from "../cache/queryKeys";
import type { VarScope } from "@/lib/vars";
import { emptyScope, interpolateArgs } from "@/lib/vars";
import { makeWidgetBridge } from "./widgetBridge";
import { VIZ_QUERY_TOOL } from "./useVizQuery";
import type { WidgetFrame } from "./federationWidget";

const DEBOUNCE_MS = 200;

/** The `viz.query` result — the canonical `frames` a data tile consumes (rows ignored here). */
interface VizQueryResult {
  frames?: WidgetFrame[];
  rows?: Array<Record<string, unknown>>;
}

/** What a data tile needs to render: the resolved frames + loading/denied state. `denied` here means
 *  "no resolvable target or the whole `viz.query` verb was denied" — a PER-TARGET deny is not surfaced
 *  as denied; it degrades to an empty frame inside `frames` (server-side), like the built-ins. */
export interface FramesState {
  frames: WidgetFrame[];
  loading: boolean;
  denied: boolean;
}

/** A throwaway client for the standalone case (no `DashboardCacheProvider`): a non-data ext tile may
 *  mount outside a dashboard — it fetches through its own bridge, not frames — so `useQuery` needs SOME
 *  client to satisfy rules-of-hooks even though we never enable it there (no ws → `enabled:false`). */
const STANDALONE_CLIENT = new QueryClient({ defaultOptions: { queries: { retry: false } } });

/** Resolve a cell's `sources[]` to frames via `viz.query`, on the shared read cache. Mirrors
 *  `useVizQuery`'s bridge/interpolation/key exactly so the two share one cache entry. Resilient to a
 *  missing `DashboardCacheProvider`: without a ws (an ext tile mounted standalone) it does no fetch and
 *  returns empty — a v2 self-fetching tile needs no frames, and a data tile only resolves under the
 *  dashboard/channel cache that supplies the ws. */
export function useVizFrames(panel: Cell, scope: VarScope = emptyScope(), refreshKey = 0): FramesState {
  const ws = useDashboardWsOptional();

  const tools = cellTools(panel);
  const toolsKey = [...tools, VIZ_QUERY_TOOL].join("|");
  // eslint-disable-next-line react-hooks/exhaustive-deps -- re-create the bridge only when the tool SET changes
  const bridge = useMemo(() => makeWidgetBridge([...tools, VIZ_QUERY_TOOL]), [toolsKey]);

  const resolvedPanel = useMemo(() => {
    const sources: Target[] = cellSources(panel).map((t) => ({
      ...t,
      args: interpolateArgs(t.args ?? {}, scope) as Record<string, unknown>,
    }));
    return { ...panel, sources };
  }, [panel, scope]);

  const hasTarget = !!cellPrimaryTarget(panel)?.tool;

  // The SAME canonical spec `useVizQuery` builds — so `vizQueryKey` collides and react-query dedupes.
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

  const debouncedSpec = useDebounced(spec, DEBOUNCE_MS);

  // Fetch only when a ws is in scope (a provider supplied it). Without one, `enabled:false` + the
  // standalone client keep the hook inert — no unscoped key, no cross-ws bleed. When a provider IS in
  // the tree, `queryClient` here is the SAME instance context resolves (both come from the provider), so
  // this and `useVizQuery` still share ONE cache entry.
  const enabled = hasTarget && ws !== null;
  const query = useQuery(
    {
      queryKey: vizQueryKey(ws ?? "", debouncedSpec),
      enabled,
      queryFn: () => bridge.call<VizQueryResult>(VIZ_QUERY_TOOL, { panel: resolvedPanel, scope }),
    },
    ws !== null ? undefined : STANDALONE_CLIENT,
  );

  if (!enabled) return { frames: [], loading: false, denied: true };
  if (!hasTarget) return { frames: [], loading: false, denied: true };
  if (query.isError) return { frames: [], loading: false, denied: true };
  const frames = Array.isArray(query.data?.frames) ? query.data!.frames! : [];
  return { frames, loading: query.isLoading, denied: false };
}
