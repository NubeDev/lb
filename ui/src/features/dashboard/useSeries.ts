// The series data hook — the widget data-binding contract in one place (dashboard scope). It
// resolves a cell `binding` to a concrete series (explicit, or the first hit of a `series.find` tag
// query), backfills history with `series.read` (state), then folds live `Sample`s from the series SSE
// stream (motion, rule 3 — never a poll). A binding the viewer isn't granted degrades to an honest
// `denied`/empty state, never a fake value (the no-mock rule applies to the UI too).

import { useEffect, useRef, useState } from "react";

import { bindingSeries, bindingTags, openSeriesStream, type Binding } from "@/lib/dashboard";
import { findSeries, readSamples } from "@/lib/ingest/ingest.api";
import type { Facet, Sample } from "@/lib/ingest/ingest.types";
import type { DashboardSearch } from "@/features/routing/search";

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

/** Resolve `binding`, backfill its history, and keep it live. Re-runs when the binding changes. */
export function useSeries(binding: Binding, range?: DashboardSearch): SeriesState {
  const [state, setState] = useState<SeriesState>({
    series: bindingSeries(binding),
    samples: [],
    latest: null,
    loading: true,
    denied: false,
  });
  // Stable key so the effect re-runs only when the binding's meaning changes (not its identity).
  const key = JSON.stringify({ binding, range });
  const liveRef = useRef<{ close: () => void } | null>(null);

  useEffect(() => {
    let cancelled = false;
    liveRef.current?.close();
    liveRef.current = null;
    setState((s) => ({ ...s, loading: true, denied: false }));

    (async () => {
      // 1) Resolve the concrete series — explicit, or the first hit of the tag query.
      let series = bindingSeries(binding);
      if (series === null) {
        const tags = bindingTags(binding);
        try {
          const hits = await findSeries(tags.map(tagFacet));
          series = hits[0]?.replace(/^series:/, "") ?? null;
        } catch {
          series = null;
        }
      }
      if (cancelled) return;
      if (series === null) {
        setState({ series: null, samples: [], latest: null, loading: false, denied: true });
        return;
      }

      // 2) Backfill history (state). A deny/absence → honest empty/denied, never a fake value.
      try {
        const samples = await readSamples(series);
        if (cancelled) return;
        setState({
          series,
          samples: samples.slice(-BACKFILL),
          latest: samples.length ? samples[samples.length - 1] : null,
          loading: false,
          denied: false,
        });
      } catch {
        if (cancelled) return;
        setState({ series, samples: [], latest: null, loading: false, denied: true });
        return;
      }

      // 3) Go live (motion) — fold each streamed sample into the tail, de-duped by seq.
      liveRef.current = openSeriesStream(series, (sample) => {
        if (cancelled || sample.series !== series) return;
        setState((s) => {
          if (s.samples.some((x) => x.seq === sample.seq)) return s;
          const samples = [...s.samples, sample].slice(-BACKFILL);
          return { ...s, samples, latest: sample };
        });
      });
    })();

    return () => {
      cancelled = true;
      liveRef.current?.close();
      liveRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);

  return state;
}
