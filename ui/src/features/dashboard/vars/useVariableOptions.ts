// Resolve a query/source variable's options over the host-mediated bridge (widget-config-vars Slice 2).
// The resolver is the SAME `{tool,args}` a cell source uses, leashed by the variable's own tool ∩ grant
// and re-checked at the host per call (rule 5). A `custom`/`interval` variable resolves synchronously
// (static); `text`/`const` have no list. Re-resolves when `refreshKey` bumps (Slice 4 auto-refresh).
//
// One hook per file (FILE-LAYOUT). No token, no DB handle — only `bridge.call(tool, args)`.

import { useEffect, useState } from "react";

import type { Variable } from "@/lib/vars";
import { makeWidgetBridge } from "../builder/widgetBridge";
import { isQueryVariable, rowsToOptions, staticOptions, type VarOption } from "./resolveOptions";

export interface VariableOptions {
  options: VarOption[];
  loading: boolean;
  /** A denied/failed query resolves to an empty list + this flag — never a fake catalogue. */
  denied: boolean;
}

/** Resolve `variable`'s options. `refreshKey` re-runs a query variable (auto-refresh ticks bump it). */
export function useVariableOptions(variable: Variable, refreshKey = 0): VariableOptions {
  const [state, setState] = useState<VariableOptions>(() => ({
    options: staticOptions(variable),
    loading: isQueryVariable(variable),
    denied: false,
  }));

  // Re-key on the variable identity + its resolver so an edit re-resolves.
  const key = `${variable.name}:${variable.type}:${JSON.stringify(variable.query ?? null)}:${refreshKey}`;

  useEffect(() => {
    if (!isQueryVariable(variable)) {
      setState({ options: staticOptions(variable), loading: false, denied: false });
      return;
    }
    let cancelled = false;
    setState((s) => ({ ...s, loading: true }));
    const tool = variable.query!.tool;
    // Leash the bridge to the variable's own tool — the host re-checks the cap + workspace regardless.
    const bridge = makeWidgetBridge([tool]);
    (async () => {
      try {
        const result = await bridge.call(tool, (variable.query!.args as Record<string, unknown>) ?? {});
        if (cancelled) return;
        setState({ options: rowsToOptions(result), loading: false, denied: false });
      } catch {
        if (cancelled) return;
        // A deny (or any failure) is an honest empty list — never a fabricated option set.
        setState({ options: [], loading: false, denied: true });
      }
    })();
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);

  return state;
}
