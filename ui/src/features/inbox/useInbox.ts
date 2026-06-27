// The inbox hook — data + state for the real inbox view (collaboration scope, slice 4). Lists the
// durable items of an inbox channel and resolves them (approve/reject/defer). This is the REAL
// `lb-inbox` surface — the S6 approval gate as a UI action — not the workflow fake. One hook per file.

import { useCallback, useEffect, useState } from "react";

import { listInbox, resolveInbox } from "@/lib/inbox/inbox.api";
import type { Decision, Item } from "@/lib/inbox/inbox.types";

export interface InboxState {
  items: Item[];
  error: string | null;
  refresh: () => Promise<void>;
  resolve: (item: string, decision: Decision) => Promise<void>;
}

/** Drive the inbox list + resolve for `channel` (within the session workspace). Reloads after a
 *  resolve so the view reflects the decision. */
export function useInbox(channel: string): InboxState {
  const [items, setItems] = useState<Item[]>([]);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setItems(await listInbox(channel));
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [channel]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const resolve = useCallback(
    async (item: string, decision: Decision) => {
      try {
        await resolveInbox(item, decision);
        await refresh();
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [refresh],
  );

  return { items, error, refresh, resolve };
}
