// The insight DETAIL hook — fetches one record + the first page of its occurrence ring over the
// injected `InsightsClient`, and exposes ack/resolve that re-fetch on success. The shell's
// `InsightDetail` component logic, extracted so a widget's "expanded" view reuses it.

import { useCallback, useEffect, useRef, useState } from "react";

import type { Insight, InsightsClient, OccurrencePage } from "./types";

export interface InsightDetailState {
  insight: Insight | null;
  occurrences: OccurrencePage | null;
  error: string | null;
  loading: boolean;
  /** Ack/resolve-in-flight action, or null when idle. */
  actingOn: "ack" | "resolve" | null;
  refresh: () => void;
  act: (action: "ack" | "resolve") => Promise<void>;
}

/** Load + drive the detail for insight `id` over `client`. Re-fetches on `id` change and after an
 *  ack/resolve lands (so the pane re-opens with the new status). `occLimit` bounds the occurrence page
 *  (default 50). `client` is read through a ref (host-stability — see `useInsights`). */
export function useInsight(
  client: InsightsClient,
  id: string,
  occLimit = 50,
): InsightDetailState {
  const [insight, setInsight] = useState<Insight | null>(null);
  const [occurrences, setOccurrences] = useState<OccurrencePage | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [actingOn, setActingOn] = useState<"ack" | "resolve" | null>(null);
  // Bumped after an ack/resolve so the effect re-fetches the record with its new status.
  const [version, setVersion] = useState(0);

  const clientRef = useRef(client);
  clientRef.current = client;

  useEffect(() => {
    let cancelled = false;
    (async () => {
      setError(null);
      setLoading(true);
      try {
        const [row, occ] = await Promise.all([
          clientRef.current.get(id),
          clientRef.current.occurrences(id, undefined, occLimit),
        ]);
        if (cancelled) return;
        setInsight(row);
        setOccurrences(occ);
      } catch (e) {
        if (cancelled) return;
        setError(e instanceof Error ? e.message : String(e));
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [id, occLimit, version]);

  const refresh = useCallback(() => setVersion((v) => v + 1), []);

  const act = useCallback(
    async (action: "ack" | "resolve") => {
      setActingOn(action);
      setError(null);
      try {
        if (action === "ack") await clientRef.current.ack(id);
        else await clientRef.current.resolve(id);
        setVersion((v) => v + 1);
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      } finally {
        setActingOn(null);
      }
    },
    [id],
  );

  return { insight, occurrences, error, loading, actingOn, refresh, act };
}
