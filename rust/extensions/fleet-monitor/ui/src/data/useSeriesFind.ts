import { useCallback, useEffect, useState } from "react";

import { useBridge } from "@/app/useBridge";
import type { AsyncState, SeriesRef } from "./series.types";

/** Load the fleet's series via the granted `series.find` verb. Returns the lifecycle + a refetch.
 *  A rejected bridge call (out of scope / denied / no data) surfaces honestly as an error state. */
export function useSeriesFind(tags: string[] = []) {
  const bridge = useBridge();
  const [state, setState] = useState<AsyncState<SeriesRef[]>>({ status: "loading" });

  const load = useCallback(() => {
    let cancelled = false;
    setState({ status: "loading" });
    bridge
      .call<SeriesRef[]>("series.find", { tags })
      .then((rows) => {
        if (cancelled) return;
        setState({ status: "ready", data: Array.isArray(rows) ? rows : [] });
      })
      .catch((e: unknown) => {
        if (cancelled) return;
        setState({ status: "error", error: e instanceof Error ? e.message : String(e) });
      });
    return () => {
      cancelled = true;
    };
    // tags is an array literal from callers; stringify to keep the dep stable.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [bridge, JSON.stringify(tags)]);

  useEffect(() => load(), [load]);

  return { state, refresh: load };
}
