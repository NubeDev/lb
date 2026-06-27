import { useCallback, useEffect, useState } from "react";

import { useBridge } from "@/app/useBridge";
import type { AsyncState } from "./series.types";
import type { InboxItem, InboxListResult } from "./workflow.types";

/** Load a channel's durable inbox items via the granted `inbox.list` verb. The host returns
 *  `{ items: Item[] }`; this hook unwraps the array. An empty result is the HONEST state when the node
 *  produced no items for this channel — the page shows an empty list, never a fabricated triage item.
 *  A rejected call (out of scope / denied) surfaces honestly as an error. */
export function useInboxList(channel: string) {
  const bridge = useBridge();
  const [state, setState] = useState<AsyncState<InboxItem[]>>({ status: "idle" });

  const load = useCallback(() => {
    let cancelled = false;
    setState({ status: "loading" });
    bridge
      .call<InboxListResult>("inbox.list", { channel })
      .then((res) => {
        if (cancelled) return;
        setState({ status: "ready", data: Array.isArray(res?.items) ? res.items : [] });
      })
      .catch((e: unknown) => {
        if (!cancelled)
          setState({ status: "error", error: e instanceof Error ? e.message : String(e) });
      });
    return () => {
      cancelled = true;
    };
  }, [bridge, channel]);

  useEffect(() => load(), [load]);

  return { state, refresh: load };
}
