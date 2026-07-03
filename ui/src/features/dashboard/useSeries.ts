// The series data hook — the widget data-binding contract in one place (dashboard scope), now with its
// STATE read (resolve + backfill) on the dashboard READ CACHE (dashboard-query-cache-scope) and its MOTION
// (the live SSE tail) kept OUTSIDE the cache (state vs motion, README §3.3). It resolves a cell `binding`
// to a concrete series (explicit, or the first hit of a `series.find` tag query), backfills history with
// `series.read` — BOTH through one cached `useQuery` keyed on the binding, so N cells on the same series
// share ONE `series.read` (scope goal 4) — then folds live `Sample`s from the series SSE stream on top of
// that shared backfill. A binding the viewer isn't granted degrades to an honest `denied`/empty state.
//
// SSE subscriber-sharing (one EventSource per series, fanned to N cells) is the scope's DEFERRED follow-up;
// each cell still opens its own stream here. The query-cache de-dup of the backfill is the shipped win.

import { useEffect, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";

import { bindingSeries, bindingTags, openSeriesStream, type Binding } from "@/lib/dashboard";
import { findSeries, readSamples } from "@/lib/ingest/ingest.api";
import type { Facet, Sample } from "@/lib/ingest/ingest.types";
import type { DashboardSearch } from "@/features/routing/search";
import { useDashboardWs } from "./cache/useDashboardWs";
import { seriesReadKey } from "./cache/queryKeys";

/** The most recent samples a widget renders, plus the resolved series + status. */
export interface SeriesState {
  /** The resolved concrete series name (after a tag query), or `null` while resolving / unresolved. */
  series: string | null;
  samples: Sample[];
  latest: Sample | null;
  loading: boolean;
  /** True when the bound series is un-granted / not found — render an honest denied state. */
  denied: boolean;
}

/** Parse a `key:value` tag string into a `series.find` facet (key-only when no `:`). */
function tagFacet(tag: string): Facet {
  const i = tag.indexOf(":");
  return i === -1 ? { key: tag } : { key: tag.slice(0, i), value: tag.slice(i + 1) };
}

/** How many recent samples to backfill (the chart's initial range). Bounded — a fan-out cap. */
const BACKFILL = 200;

/** The resolve + backfill result the cache holds — the STATE half (motion is layered on locally). */
interface SeriesBackfill {
  series: string | null;
  samples: Sample[];
}

/** Resolve `binding` to a concrete series, then backfill its history. Both steps run inside the cached
 *  query fn so N cells on one series share one entry. A denied read rejects → an honest denied state. */
async function resolveAndBackfill(binding: Binding): Promise<SeriesBackfill> {
  let series = bindingSeries(binding);
  if (series === null) {
    const hits = await findSeries(bindingTags(binding).map(tagFacet));
    series = hits[0]?.replace(/^series:/, "") ?? null;
  }
  if (series === null) return { series: null, samples: [] };
  const samples = await readSamples(series);
  return { series, samples: samples.slice(-BACKFILL) };
}

/** Resolve `binding`, backfill its history (shared, cached), and keep it live (local SSE tail). */
export function useSeries(binding: Binding, range?: DashboardSearch): SeriesState {
  const ws = useDashboardWs();
  // Key on the binding's MEANING (not its identity) so N cells with the same binding share one entry, and
  // an unrelated re-render doesn't re-fetch. `range` participates so a range change re-keys the backfill.
  const bindingKey = JSON.stringify({ binding, range });
  const query = useQuery({
    queryKey: [...seriesReadKey(ws, bindingKey)],
    queryFn: () => resolveAndBackfill(binding),
  });

  const resolvedSeries = query.data?.series ?? null;
  // The LIVE tail (motion) — folded over the cached backfill, held in local state so it never enters the
  // cache. Re-armed whenever the resolved series changes.
  const [live, setLive] = useState<Sample[]>([]);
  const liveRef = useRef<{ close: () => void } | null>(null);

  useEffect(() => {
    setLive([]);
    liveRef.current?.close();
    liveRef.current = null;
    if (!resolvedSeries) return;
    liveRef.current = openSeriesStream(resolvedSeries, (sample) => {
      if (sample.series !== resolvedSeries) return;
      setLive((tail) => (tail.some((x) => x.seq === sample.seq) ? tail : [...tail, sample]));
    });
    return () => {
      liveRef.current?.close();
      liveRef.current = null;
    };
  }, [resolvedSeries]);

  if (query.isError) return { series: null, samples: [], latest: null, loading: false, denied: true };
  if (query.isLoading || !query.data) {
    return { series: bindingSeries(binding), samples: [], latest: null, loading: true, denied: false };
  }
  if (resolvedSeries === null) {
    return { series: null, samples: [], latest: null, loading: false, denied: true };
  }
  // Merge the cached backfill with the live tail (de-duped by seq), bounded to the backfill window.
  const merged: Sample[] = [...query.data.samples];
  for (const s of live) if (!merged.some((x) => x.seq === s.seq)) merged.push(s);
  const samples = merged.slice(-BACKFILL);
  return {
    series: resolvedSeries,
    samples,
    latest: samples.length ? samples[samples.length - 1] : null,
    loading: false,
    denied: false,
  };
}
