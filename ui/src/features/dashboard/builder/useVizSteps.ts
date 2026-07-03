// The per-step transform debug hook (editor-parity scope, step 7) — calls `viz.query` with the panel's
// `debug` flag on, so the backend returns `steps[]`: the input snapshot + the frames AFTER each applied
// transform (honoring an optional `stopAt`). Rides the SAME WidgetBridge + `viz.query` cap as the normal
// data path (no new verb). Debounced like `useVizQuery`. One responsibility: fetch the stepwise frames.

import { useEffect, useMemo, useState } from "react";

import type { Cell } from "@/lib/dashboard";
import { cellSources, cellPrimaryTarget } from "@/lib/dashboard";
import { cellTools } from "../views/WidgetView";
import type { VarScope } from "@/lib/vars";
import { emptyScope, interpolateArgs } from "@/lib/vars";
import { makeWidgetBridge } from "./widgetBridge";
import { VIZ_QUERY_TOOL } from "./useVizQuery";

/** A canonical column frame, as `viz.query` returns it. */
export interface StepFrame {
  refId?: string;
  name?: string;
  fields: Array<{ name: string; type?: string; values: unknown[] }>;
  length?: number;
}

/** One pipeline snapshot: the applied step index (null = the pre-pipeline input) + its frames. */
export interface Step {
  step: number | null;
  frames: StepFrame[];
}

interface StepsState {
  steps: Step[];
  loading: boolean;
  denied: boolean;
}

const DEBOUNCE_MS = 200;

/** Resolve the panel's per-step debug frames. `stopAt`, when set, bounds the number of applied steps.
 *  Enabled only when `enabled` — the Transform tab turns it on when the debug view is open. */
export function useVizSteps(
  panel: Cell,
  enabled: boolean,
  scope: VarScope = emptyScope(),
  refreshKey = 0,
  stopAt?: number,
): StepsState {
  const [state, setState] = useState<StepsState>({ steps: [], loading: false, denied: false });

  const tools = cellTools(panel);
  const toolsKey = [...tools, VIZ_QUERY_TOOL].join("|");
  // eslint-disable-next-line react-hooks/exhaustive-deps -- re-create the bridge only when the tool SET changes
  const bridge = useMemo(() => makeWidgetBridge([...tools, VIZ_QUERY_TOOL]), [toolsKey]);

  const resolvedPanel = useMemo(() => {
    const sources = cellSources(panel).map((t) => ({
      ...t,
      args: interpolateArgs(t.args ?? {}, scope) as Record<string, unknown>,
    }));
    // The `debug` flag turns on the stepwise resolver; `stopAt` bounds applied steps.
    const debug = stopAt === undefined ? true : { stopAt };
    return { ...panel, sources, debug };
  }, [panel, scope, stopAt]);

  const key = useMemo(() => JSON.stringify({ resolvedPanel, scope, refreshKey }), [resolvedPanel, scope, refreshKey]);
  const hasTarget = !!cellPrimaryTarget(panel)?.tool;

  useEffect(() => {
    if (!enabled || !hasTarget) {
      setState({ steps: [], loading: false, denied: false });
      return;
    }
    let cancelled = false;
    setState((s) => ({ ...s, loading: true, denied: false }));
    const timer = setTimeout(() => {
      (async () => {
        try {
          const result = await bridge.call<{ steps?: Step[] }>(VIZ_QUERY_TOOL, { panel: resolvedPanel, scope });
          if (cancelled) return;
          setState({ steps: Array.isArray(result?.steps) ? result.steps : [], loading: false, denied: false });
        } catch {
          if (cancelled) return;
          setState({ steps: [], loading: false, denied: true });
        }
      })();
    }, DEBOUNCE_MS);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- `key` already encodes panel + scope + tick
  }, [key, bridge, enabled, hasTarget]);

  return state;
}
