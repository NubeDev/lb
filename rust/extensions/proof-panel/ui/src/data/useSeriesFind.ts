import { useCallback, useEffect, useState } from "react";

import { useBridge } from "@/app/useBridge";
import type { AsyncState, Facet, SeriesFindResult } from "./series.types";

/** Load the workspace's series via the granted `series.find` verb. The host verb takes a `facets`
 *  array and returns `{ series: string[] }` (names); this hook sends the facets and unwraps the names,
 *  so callers get a plain `string[]`. A rejected bridge call (out of scope / denied) surfaces honestly
 *  as an error state — never a fabricated list.
 *
 *  NOTE: an EMPTY `facets` query returns NOTHING from the host (a query must constrain something — see
 *  `lb_tags::find`). The page therefore lists series by searching a facet; with no facet the honest
 *  result is "no series to show" until the user constrains the query. */
export function useSeriesFind(facets: Facet[]) {
  const bridge = useBridge();
  const [state, setState] = useState<AsyncState<string[]>>({ status: "idle" });

  const load = useCallback(() => {
    // No facets → nothing to query; stay idle (the page shows the "search to list" prompt). This avoids
    // a guaranteed-empty round-trip and keeps the empty state honest rather than misleading.
    if (facets.length === 0) {
      setState({ status: "idle" });
      return () => {};
    }
    let cancelled = false;
    setState({ status: "loading" });
    bridge
      .call<SeriesFindResult>("series.find", { facets })
      .then((res) => {
        if (cancelled) return;
        const names = Array.isArray(res?.series) ? res.series : [];
        setState({ status: "ready", data: names });
      })
      .catch((e: unknown) => {
        if (cancelled) return;
        setState({ status: "error", error: e instanceof Error ? e.message : String(e) });
      });
    return () => {
      cancelled = true;
    };
    // facets is an array literal from callers; stringify to keep the dep stable.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [bridge, JSON.stringify(facets)]);

  useEffect(() => load(), [load]);

  return { state, refresh: load };
}
