// The insights data hook — data + state for the Insights page (insights umbrella scope). Lists
// insights newest-first, keyset-paged, with the faceted filter the sidebar owns; ack/resolve are
// the in-page actions. Live updates ride `insight.watch` SSE (TODO: wire the SSE subscription —
// the bus event is published by the host; the UI's live tail is a named follow-up).
//
// STUB: the data-fetching + keyset paging + SSE wiring are deferred to the implementing session.
// The hook returns the honest `loading`/`error`/`items` shape today (rendered as "loading…" /
// "failed to load"); the bodies are TODO so the page renders without lying about state.

import { useCallback, useEffect, useState } from "react";

import { listInsights } from "@/lib/insights/insights.api";
import type { Insight, ListQuery } from "@/lib/insights/insights.types";

export interface InsightsState {
  items: Insight[];
  error: string | null;
  loading: boolean;
  /** Ack-in-flight item id, or null when idle (per-row disable + spin, the inbox pattern). */
  actingOn: string | null;
  refresh: () => Promise<void>;
  setFilter: (filter: ListQuery) => void;
  /**
   * Ack/resolve dispatcher (TODO: the implementing session wires the per-row disable + spin +
   * error surfacing through this). Held in the hook so the row components stay presentational.
   * Today this is a stub that flips `actingOn` and re-fetches; the real impl calls `ackInsight`/
   * `resolveInsight` through `insights.api.ts`.
   */
  act: (id: string, action: "ack" | "resolve") => Promise<void>;
}

/**
 * Drive the Insights list for the session workspace. `initial` is the starting filter (status /
 * severity / tags / range); `setFilter` swaps it. TODO: keyset paging (load-more on `next` cursor),
 * `insight.watch` SSE subscription, and the ack/resolve actions (the action buttons call through
 * `insights.api.ts` directly today; consolidating them here is the implementing session's call).
 */
export function useInsights(initial: ListQuery): InsightsState {
  const [items, setItems] = useState<Insight[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  // `actingOn` is part of the hook's public state; the implementing session wires the per-row
  // ack/resolve here (the inbox `resolving` pattern). Held as state so the row can disable + spin.
  const [actingOn, setActingOn] = useState<string | null>(null);
  const [filter, setFilterState] = useState<ListQuery>(initial);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const page = await listInsights(filter);
      // TODO: keyset paging — append on `page.next` instead of replacing when the caller asks
      // for the next page. For now the first page replaces (the common refresh path).
      setItems(page.items);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [filter]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const setFilter = useCallback((next: ListQuery) => {
    setFilterState(next);
  }, []);

  // TODO: real ack/resolve through `insights.api.ts`; today a no-op placeholder so the row
  // components have a stable hook contract. The implementing session replaces the body.
  const act = useCallback(
    async (id: string, _action: "ack" | "resolve") => {
      setActingOn(id);
      try {
        // await ackInsight(id) / resolveInsight(id) — TODO implementing session.
        await refresh();
      } finally {
        setActingOn(null);
      }
    },
    [refresh],
  );

  return { items, error, loading, actingOn, refresh, setFilter, act };
}
