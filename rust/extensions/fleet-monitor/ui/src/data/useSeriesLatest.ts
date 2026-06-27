import { useCallback, useEffect, useState } from "react";

import { useBridge } from "@/app/useBridge";
import type { AsyncState, LatestSample } from "./series.types";

/** Load the latest sample for `series` via the granted `series.latest` verb. A rejected call (out of
 *  scope / denied / no data) surfaces honestly as an error state — never fabricated alerts. */
export function useSeriesLatest(series: string | null) {
  const bridge = useBridge();
  const [state, setState] = useState<AsyncState<LatestSample | null>>({ status: "loading" });

  const load = useCallback(() => {
    if (!series) {
      setState({ status: "ready", data: null });
      return () => {};
    }
    let cancelled = false;
    setState({ status: "loading" });
    bridge
      .call<LatestSample | null>("series.latest", { series })
      .then((sample) => {
        if (!cancelled) setState({ status: "ready", data: sample ?? null });
      })
      .catch((e: unknown) => {
        if (!cancelled) setState({ status: "error", error: e instanceof Error ? e.message : String(e) });
      });
    return () => {
      cancelled = true;
    };
  }, [bridge, series]);

  useEffect(() => load(), [load]);

  return { state, refresh: load };
}
