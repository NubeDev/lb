// The insights data hook — data + state for the Insights page (insights umbrella scope). Lists
// insights newest-first, keyset-paged, with the faceted filter the sidebar owns; ack/resolve are
// the in-page actions. Live updates ride the `insight.watch` SSE feed (subscribed here; the host
// publishes the raise/ack/resolve events on `ws/{ws}/insight/events`).

import { useCallback, useEffect, useRef, useState } from "react";

import {
  ackInsight,
  listInsights,
  resolveInsight,
} from "@/lib/insights/insights.api";
import { subscribeInsightEvents } from "@/lib/insights/insights.events";
import type { Insight, ListQuery, PageCursor } from "@/lib/insights/insights.types";

export interface InsightsState {
  items: Insight[];
  error: string | null;
  loading: boolean;
  /** Ack-in-flight item id, or null when idle (per-row disable + spin, the inbox pattern). */
  actingOn: string | null;
  /** The keyset cursor for the next page, or null when the current list is the last page. */
  nextCursor: PageCursor | null;
  refresh: () => Promise<void>;
  loadMore: () => Promise<void>;
  setFilter: (filter: ListQuery) => void;
  act: (id: string, action: "ack" | "resolve") => Promise<void>;
}

/**
 * Drive the Insights list for the session workspace. `initial` is the starting filter (status /
 * severity / tags / range); `setFilter` swaps it. Keyset paging appends on `loadMore`; the
 * `insight.watch` SSE feed refreshes the head on each raise/ack/resolve.
 */
export function useInsights(initial: ListQuery): InsightsState {
  const [items, setItems] = useState<Insight[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [actingOn, setActingOn] = useState<string | null>(null);
  const [nextCursor, setNextCursor] = useState<PageCursor | null>(null);
  const [filter, setFilterState] = useState<ListQuery>(initial);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      // The head page (no cursor) — replaces the list (the common refresh path).
      const page = await listInsights({ ...filter, cursor: undefined });
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
      const page = await listInsights({ ...filter, cursor: nextCursor });
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

  // Live tail — refresh the head when the workspace raises/acks/resolves an insight. Keeps the
  // filtered list honest without a poll; a coalesced refresh is cheaper than merging events by hand.
  const refreshRef = useRef(refresh);
  refreshRef.current = refresh;
  useEffect(() => {
    const stop = subscribeInsightEvents(() => {
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
        if (action === "ack") await ackInsight(id);
        else await resolveInsight(id);
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
