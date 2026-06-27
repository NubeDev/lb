import { useCallback, useEffect, useState } from "react";

import { useBridge } from "@/app/useBridge";
import type { AsyncState } from "./series.types";
import type { OutboxStatus } from "./workflow.types";

/** Load the workspace's outbox delivery snapshot via the granted `outbox.status` verb (read-only, no
 *  args). The host returns `{ pending, delivered, dead_lettered }`; this hook surfaces it as an async
 *  state plus a `refresh`. A rejected call (out of scope / denied) surfaces honestly as an error. */
export function useOutboxStatus() {
  const bridge = useBridge();
  const [state, setState] = useState<AsyncState<OutboxStatus>>({ status: "idle" });

  const load = useCallback(() => {
    let cancelled = false;
    setState({ status: "loading" });
    bridge
      .call<OutboxStatus>("outbox.status", {})
      .then((res) => {
        if (cancelled) return;
        setState({
          status: "ready",
          data: {
            pending: res?.pending ?? [],
            delivered: res?.delivered ?? [],
            dead_lettered: res?.dead_lettered ?? [],
          },
        });
      })
      .catch((e: unknown) => {
        if (!cancelled)
          setState({ status: "error", error: e instanceof Error ? e.message : String(e) });
      });
    return () => {
      cancelled = true;
    };
  }, [bridge]);

  useEffect(() => load(), [load]);

  return { state, refresh: load };
}
