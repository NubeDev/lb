// The insights LIST hook — data + state for a list of insights (the page, a widget). Lists newest-
// first, keyset-paged, over the injected `InsightsClient`; ack/resolve are the in-place actions. Live
// updates ride the client's OPTIONAL `subscribe` feed (folded into a head refresh); a client with no
// feed simply updates on the act→refresh round trip.
//
// This is the shipped shell `useInsights` with its `@/lib/*` imports replaced by an `InsightsClient`
// param — so it runs from the shell, a dashboard widget, and an extension bridge alike.

import { useCallback, useEffect, useRef, useState } from "react";

import type { Insight, InsightsClient, ListQuery, PageCursor } from "./types";

export interface InsightsState {
  items: Insight[];
  error: string | null;
  loading: boolean;
  /** Ack/resolve-in-flight item id, or null when idle (per-row disable + spin, the inbox pattern). */
  actingOn: string | null;
  /** The keyset cursor for the next page, or null when the current list is the last page. */
  nextCursor: PageCursor | null;
  refresh: () => Promise<void>;
  loadMore: () => Promise<void>;
  setFilter: (filter: ListQuery) => void;
  act: (id: string, action: "ack" | "resolve") => Promise<void>;
}

/** Drive an insights list over `client`. `initial` is the starting filter (status / severity / tags /
 *  range); `setFilter` swaps it. Keyset paging appends on `loadMore`; the client's `subscribe` feed (if
 *  any) refreshes the head on each raise/ack/resolve. `client` is read through a ref so an unmemoized
 *  literal per render does not loop (the source-picker host-stability guarantee). */
export function useInsights(client: InsightsClient, initial: ListQuery): InsightsState {
  const [items, setItems] = useState<Insight[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [actingOn, setActingOn] = useState<string | null>(null);
  const [nextCursor, setNextCursor] = useState<PageCursor | null>(null);
  const [filter, setFilterState] = useState<ListQuery>(initial);

  const clientRef = useRef(client);
  clientRef.current = client;

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      // The head page (no cursor) — replaces the list (the common refresh path).
      const page = await clientRef.current.list({ ...filter, cursor: undefined });
      setItems(page.items);
      setNextCursor(page.next ?? null);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [filter]);

  const loadMore = useCallback(async () => {
    if (!nextCursor) return;
    setLoading(true);
    try {
      const page = await clientRef.current.list({ ...filter, cursor: nextCursor });
      // Append the next keyset page (dedup by id — a concurrent raise could overlap the boundary).
      setItems((prev) => {
        const seen = new Set(prev.map((i) => i.id));
        return [...prev, ...page.items.filter((i) => !seen.has(i.id))];
      });
      setNextCursor(page.next ?? null);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [filter, nextCursor]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  // Live tail — refresh the head when the workspace raises/acks/resolves an insight. A coalesced
  // refresh is cheaper than merging events by hand; a client with no `subscribe` gets a no-op.
  const refreshRef = useRef(refresh);
  refreshRef.current = refresh;
  useEffect(() => {
    const subscribe = clientRef.current.subscribe;
    if (!subscribe) return;
    const stop = subscribe(() => {
      void refreshRef.current();
    });
    return stop;
  }, []);

  const setFilter = useCallback((next: ListQuery) => {
    setFilterState(next);
  }, []);

  const act = useCallback(
    async (id: string, action: "ack" | "resolve") => {
      setActingOn(id);
      try {
        if (action === "ack") await clientRef.current.ack(id);
        else await clientRef.current.resolve(id);
        await refresh();
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      } finally {
        setActingOn(null);
      }
    },
    [refresh],
  );

  return {
    items,
    error,
    loading,
    actingOn,
    nextCursor,
    refresh,
    loadMore,
    setFilter,
    act,
  };
}
