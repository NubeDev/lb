// The outbox hook — data + state for the read-only delivery status view (collaboration scope, slice
// 4). Reads the workspace's effects grouped pending / delivered / dead-lettered / held. Read-only:
// there is no mutation verb (the outbox is must-deliver infrastructure). One hook per file
// (FILE-LAYOUT).
//
// Mirrors `useInbox`'s honest-state contract: `loading` lets the header Refresh button spin and the
// empty state distinguish "loading" from "empty", while `error` is surfaced by the view's banner.

import { useCallback, useEffect, useState } from "react";

import { outboxStatus } from "@/lib/outbox/outbox.api";
import type { OutboxStatus } from "@/lib/outbox/outbox.types";

const EMPTY: OutboxStatus = { pending: [], delivered: [], dead_lettered: [] };

export interface OutboxState {
  status: OutboxStatus;
  error: string | null;
  /** True while a status fetch (initial load or manual refresh) is in flight. */
  loading: boolean;
  refresh: () => Promise<void>;
}

/** Drive the outbox status snapshot for the session workspace. Re-read with `refresh`. */
export function useOutbox(): OutboxState {
  const [status, setStatus] = useState<OutboxStatus>(EMPTY);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      setStatus(await outboxStatus());
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return { status, error, loading, refresh };
}
