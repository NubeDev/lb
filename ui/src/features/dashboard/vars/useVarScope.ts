// Resolve the dashboard's VarScope shell-side (widget-config-vars Slice 3). The scope = the resolved
// user-variable selections (from the URL, falling back to a definition's default) + the built-ins
// (`$__from`/`${__user.*}`/`${__workspace}`/…), resolved from the VERIFIED session + the URL time range.
// The cell/iframe never resolves identity or query vars itself — the shell builds this and hands cells
// resolved values (the un-spoofable invariant). One hook per file.
//
// `values` resolution per variable: the URL selection if present; else a `const`/`text` default; else
// omitted (an unreferenced/unselected variable is left literal by `interpolate`). Query-option defaulting
// (Grafana's "first option") is the bar's job — selection flows through the URL, not re-resolved here.

import { useMemo } from "react";

import type { Variable, VarScope } from "@/lib/vars";
import { resolveBuiltins } from "@/lib/vars";
import { getSession } from "@/lib/session";
import type { DashboardSearch } from "@/features/routing/search";
import { varsFromSearch } from "@/features/routing/search";

/** Parse an ISO `yyyy-mm-dd` (the dashboard range) to epoch ms at UTC midnight; `null` if malformed. */
function isoToMs(iso: string | undefined): number | null {
  if (!iso || !/^\d{4}-\d{2}-\d{2}$/.test(iso)) return null;
  const ms = Date.parse(`${iso}T00:00:00.000Z`);
  return Number.isNaN(ms) ? null : ms;
}

/** The login (local part of `user:ada` → `ada`) for `${__user.login}`. */
function loginOf(principal: string): string {
  const i = principal.indexOf(":");
  return i >= 0 ? principal.slice(i + 1) : principal;
}

/** Build the resolved VarScope for a dashboard. `variables` are the definitions (defaults); `search` is
 *  the URL (selections + time range); `dashboardId`/`workspace` feed the built-ins. */
export function useVarScope(
  variables: Variable[],
  search: DashboardSearch | undefined,
  dashboardId: string,
  workspace: string,
): VarScope {
  // Re-key on the inputs that change the scope (definitions, selection, range, dashboard, workspace).
  const selection = search ? varsFromSearch(search) : {};
  const key = JSON.stringify([variables, selection, search?.from, search?.to, dashboardId, workspace]);

  return useMemo(() => {
    const values: Record<string, string | string[]> = {};
    for (const v of variables) {
      if (selection[v.name] !== undefined) {
        values[v.name] = selection[v.name];
      } else if (v.type === "const" && v.const) {
        values[v.name] = v.const;
      } else if (v.type === "text" && v.text) {
        values[v.name] = v.text;
      } else if (v.type === "interval" && v.interval?.length) {
        values[v.name] = v.interval[0]; // the default interval (feeds $__interval)
      }
      // A query/source/custom variable with no selection is left out (interpolate leaves it literal).
    }

    const session = getSession();
    // The interval built-in tracks the first interval-type variable's resolved value, if any.
    const intervalVar = variables.find((v) => v.type === "interval");
    const interval = intervalVar ? (values[intervalVar.name] as string | undefined) : undefined;

    const fromMs = isoToMs(search?.from);
    const toMs = isoToMs(search?.to);
    const builtins = resolveBuiltins({
      timeRange: fromMs !== null && toMs !== null ? { fromMs, toMs } : undefined,
      identity: session ? { login: loginOf(session.principal) } : undefined,
      dashboardId,
      workspace,
      interval,
    });

    return { values, builtins } satisfies VarScope;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);
}
