// The Phase-3 viz.query fetch effect — the data path behind the ONE panel hook (viz README phasing,
// invariant A). It calls the backend host verb `viz.query` through the SAME WidgetBridge a cell already
// rides (`mcp:viz.query:call`, qualified tool `"viz.query"`), passing the WHOLE cell as `panel` (so the
// server has every `sources[]`/`transformations[]`/`source`/`fieldConfig`) and the resolved VarScope as
// `scope`. The backend runs the transform pipeline (`lb-viz`) and returns `{ frames, rows }`; we map
// `rows` → the SAME `SourceState` shape `useSource` produces, so every renderer + the editor preview are
// UNCHANGED (invariant B: NO client-side transform execution).
//
// The call is DEBOUNCED (~200ms) so editor edits don't spam the gateway. A denied/missing cap → the
// bridge throws → an honest `denied` state, never a fabricated value (CLAUDE §9). One responsibility:
// resolve a non-watch panel's rows via viz.query.

import { useEffect, useMemo, useState } from "react";

import type { Cell, Target } from "@/lib/dashboard";
import { cellPrimaryTarget, cellSources } from "@/lib/dashboard";
import { cellTools } from "../views/WidgetView";
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

/** The `viz.query` result — canonical `frames` PLUS the primary frame flattened to `rows` (the SAME row
 *  shape the renderers already consume, so they're unchanged). */
interface VizQueryResult {
  frames?: Frame[];
  rows?: Array<Record<string, unknown>>;
}

/** Pull a scalar "latest" from the rows (newest row's `value`/`payload`, or the row itself) — mirrors
 *  `useSource.toLatest` so a stat/gauge reads the same scalar whether the rows came via viz.query or the
 *  live path. */
function toLatest(rows: Array<Record<string, unknown>>): unknown {
  if (!rows.length) return null;
  const last = rows[rows.length - 1];
  return last.value ?? last.payload ?? last;
}

/** Resolve a panel's rows via the backend `viz.query` verb. The cell goes over as `panel` whole; the
 *  resolved scope as `scope`. Debounced so editor keystrokes don't spam. Re-runs when the panel spec,
 *  scope, tool set, or refresh tick changes. */
export function useVizQuery(panel: Cell, scope: VarScope = emptyScope(), refreshKey = 0): SourceState {
  const [state, setState] = useState<SourceState>({
    rows: [],
    latest: null,
    loading: true,
    denied: false,
  });

  // The bridge leash = the cell's tool set ∩ grant, PLUS viz.query itself (the cell calls it, so it must
  // be forwardable). Re-create the bridge only when that set changes.
  const tools = cellTools(panel);
  const toolsKey = [...tools, VIZ_QUERY_TOOL].join("|");
  // eslint-disable-next-line react-hooks/exhaustive-deps -- re-create the bridge only when the tool SET changes
  const bridge = useMemo(() => makeWidgetBridge([...tools, VIZ_QUERY_TOOL]), [toolsKey]);

  // Interpolate each target's args against the resolved scope BEFORE the call — a `${host}` in a target
  // repoints the series exactly as the shipped `useSource` did pre-call (the host's parse-allowlist stays
  // the boundary; only safe arg leaves are substituted, never string-spliced SQL). The host still gets
  // `scope` too, so `lb-viz` can interpolate TRANSFORM option values. The panel sent over carries the
  // interpolated `sources[]` so the resolver dispatches the resolved query.
  const resolvedPanel = useMemo(() => {
    const sources: Target[] = cellSources(panel).map((t) => ({
      ...t,
      args: interpolateArgs(t.args ?? {}, scope) as Record<string, unknown>,
    }));
    return { ...panel, sources };
  }, [panel, scope]);

  // Re-key on the RESOLVED panel spec + scope + refresh tick — a transform/target/var edit re-runs it.
  const key = useMemo(
    () => JSON.stringify({ resolvedPanel, scope, refreshKey }),
    [resolvedPanel, scope, refreshKey],
  );

  // A panel with no resolvable primary target has nothing to query — an honest empty/denied state, no
  // wasted gateway round-trip (also the live-path placeholder's resting state in `usePanelData`).
  const hasTarget = !!cellPrimaryTarget(panel)?.tool;

  useEffect(() => {
    if (!hasTarget) {
      setState({ rows: [], latest: null, loading: false, denied: true });
      return;
    }
    let cancelled = false;
    setState((s) => ({ ...s, loading: true, denied: false }));

    const timer = setTimeout(() => {
      (async () => {
        try {
          const result = await bridge.call<VizQueryResult>(VIZ_QUERY_TOOL, { panel: resolvedPanel, scope });
          if (cancelled) return;
          const rows = Array.isArray(result?.rows) ? result.rows : [];
          setState({ rows, latest: toLatest(rows), loading: false, denied: false });
        } catch {
          if (cancelled) return;
          // A missing cap / denied target throws (opaque) → honest denied, never a fabricated value.
          setState({ rows: [], latest: null, loading: false, denied: true });
        }
      })();
    }, DEBOUNCE_MS);

    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- `key` already encodes the panel spec + scope + tick
  }, [key, bridge]);

  return state;
}
