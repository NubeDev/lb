import { useCallback, useEffect, useState } from "react";

import { useBridge } from "@/app/useBridge";
import type { AsyncState, Sample, SeriesLatestResult } from "./series.types";

/** Load the latest sample of `series` via the granted `series.latest` verb. The host verb returns
 *  `{ sample: Sample | null }`; this hook unwraps it. A rejected call (out of scope / denied) surfaces
 *  honestly as an error state — the exact path the grant-intersection deny-test exercises: when the
 *  install approval omitted `series.latest`, this call is denied at the bridge (403), and the page
 *  shows the error rather than a blank or a fabricated value. */
export function useSeriesLatest(series: string | null) {
  const bridge = useBridge();
  const [state, setState] = useState<AsyncState<Sample | null>>({ status: "idle" });

  const load = useCallback(() => {
    if (!series) {
      setState({ status: "idle" });
      return () => {};
    }
    let cancelled = false;
    setState({ status: "loading" });
    bridge
      .call<SeriesLatestResult>("series.latest", { series })
      .then((res) => {
        if (!cancelled) setState({ status: "ready", data: res?.sample ?? null });
      })
      .catch((e: unknown) => {
        if (!cancelled)
          setState({ status: "error", error: e instanceof Error ? e.message : String(e) });
      });
    return () => {
      cancelled = true;
    };
  }, [bridge, series]);

  useEffect(() => load(), [load]);

  return { state, refresh: load };
}
