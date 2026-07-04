// The inbox hook — data + state for the real inbox view (collaboration scope, slice 4). Lists the
// durable items of an inbox channel and resolves them (approve/reject/defer). This is the REAL
// `lb-inbox` surface — the S6 approval gate as a UI action — not the workflow fake. One hook per file.
//
// Failure is surfaced honestly at the item that was clicked (`errors[itemId]`), not just as a single
// banner: a slow or rejected resolve used to look identical to "nothing happened", so the view now
// reads `resolving` to disable + spin the in-flight button and `errors[id]` to show the cause on the
// row/pane that owns it.

import { useCallback, useEffect, useState } from "react";

import { listInbox, resolveInbox } from "@/lib/inbox/inbox.api";
import type { Decision, Item } from "@/lib/inbox/inbox.types";

export interface InboxState {
  items: Item[];
  error: string | null;
  /** True while the initial list load (or a manual refresh) is in flight. */
  loading: boolean;
  /** The item id currently being resolved, or null when idle. */
  resolving: string | null;
  /** Per-item resolve errors keyed by item id — shown on the row/pane that owns the item. */
  errors: Record<string, string>;
  refresh: () => Promise<void>;
  resolve: (item: string, decision: Decision) => Promise<void>;
}

/** Drive the inbox list + resolve for `channel` (within the session workspace). Reloads after a
 *  resolve so the view reflects the decision. */
export function useInbox(channel: string): InboxState {
  const [items, setItems] = useState<Item[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [resolving, setResolving] = useState<string | null>(null);
  const [errors, setErrors] = useState<Record<string, string>>({});

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      setItems(await listInbox(channel));
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [channel]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const resolve = useCallback(
    async (item: string, decision: Decision) => {
      setResolving(item);
      // Clear any prior per-item error so the row/pane stops showing a stale failure.
      setErrors((prev) => (prev[item] ? { ...prev, [item]: "" } : prev));
      try {
        await resolveInbox(item, decision);
        await refresh();
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        setError(msg);
        setErrors((prev) => ({ ...prev, [item]: msg }));
      } finally {
        setResolving(null);
      }
    },
    [refresh],
  );

  return { items, error, loading, resolving, errors, refresh, resolve };
}
