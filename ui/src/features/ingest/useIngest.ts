// The Ingest-page hook — data + state for the series explorer (data-console scope). Lists/searches
// the workspace's series, loads a selected series' latest + recent samples, and pushes one sample by
// hand. Snapshot reads + manual refresh (no live motion — that is the dashboard's job). One hook per
// file (FILE-LAYOUT). Everything runs against the real gateway; the producer is the token's principal.

import { useCallback, useEffect, useState } from "react";

import { findSeries, latestSample, readSamples, writeSample } from "@/lib/ingest/ingest.api";
import { listRealSeries, loadSchema, saveSchema } from "@/lib/ingest/schema.api";
import type { Facet, Sample } from "@/lib/ingest/ingest.types";
import type { SeriesSchema } from "@/lib/ingest/schema.types";

/** How many recent samples the detail table shows per page (the tail window). */
const PAGE = 10;

export interface IngestState {
  series: string[];
  selected: string | null;
  /** The selected series' schema (drives the typed write form), or `null` if it has none. */
  schema: SeriesSchema | null;
  latest: Sample | null;
  recent: Sample[];
  /** The current 0-based page of the recent-samples table (page 0 = newest). */
  page: number;
  /** Whether an older page exists (i.e. samples older than the current window). */
  hasOlder: boolean;
  error: string | null;
  /** Refresh the series list — `query` is a `key:value` facet filter, or a plain prefix, or empty. */
  search: (query: string) => Promise<void>;
  /** Select a series and load its schema + latest + recent samples. */
  select: (series: string) => Promise<void>;
  /** Jump the recent-samples table to `page` (0 = newest); clamps at the ends. */
  goToPage: (page: number) => Promise<void>;
  /** Push one sample (the manual-write form), then refresh the selected series' table. */
  write: (sample: Sample) => Promise<void>;
  /** Create a new series from the wizard: persist its schema, refresh the list, and select it. */
  create: (schema: SeriesSchema) => Promise<void>;
}

/** Parse a search box value into a `series.find` facet (`key:value` / `key:`) or `null` for a plain
 *  prefix list. `host:pi-7` → exact facet; `region:` → key-only; `node` → prefix (null). */
function parseFacet(query: string): Facet | null {
  const q = query.trim();
  if (!q.includes(":")) return null;
  const [key, ...rest] = q.split(":");
  const value = rest.join(":").trim();
  return value ? { key, value } : { key };
}

/** Drive the Ingest page for the session workspace. */
export function useIngest(): IngestState {
  const [series, setSeries] = useState<string[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [schema, setSchema] = useState<SeriesSchema | null>(null);
  const [latest, setLatest] = useState<Sample | null>(null);
  const [recent, setRecent] = useState<Sample[]>([]);
  const [page, setPage] = useState(0);
  const [hasOlder, setHasOlder] = useState(false);
  const [error, setError] = useState<string | null>(null);

  /** Read one page (newest-first) of `series`, given its newest seq. Page 0 is the newest `PAGE`
   *  samples; each later page steps `PAGE` further back. Fetches only that window from the gateway
   *  (a bounded `series.read` range), never the whole tail. */
  const loadPage = useCallback(async (s: string, topSeq: number, pageIdx: number) => {
    const hi = topSeq - pageIdx * PAGE;
    const lo = Math.max(0, hi - PAGE + 1);
    if (hi < 0) {
      setRecent([]);
      setHasOlder(false);
      return;
    }
    const rows = await readSamples(s, lo, hi);
    // Seq-ascending from the read; the table shows newest-first.
    setRecent(rows.slice().reverse());
    setHasOlder(lo > 0);
  }, []);

  const search = useCallback(async (query: string) => {
    try {
      const facet = parseFacet(query);
      // A `key:value` query is tag-faceted discovery; a plain string is a prefix list (real series
      // only — the reserved `__schema.*` meta-series stay hidden).
      setSeries(facet ? await findSeries([facet]) : await listRealSeries(query.trim()));
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  const select = useCallback(
    async (s: string) => {
      setSelected(s);
      setPage(0);
      try {
        const [sch, last] = await Promise.all([loadSchema(s), latestSample(s)]);
        setSchema(sch);
        setLatest(last);
        // Only the newest page of samples — a bounded read, not the whole tail.
        await loadPage(s, last?.seq ?? -1, 0);
        setError(null);
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [loadPage],
  );

  const goToPage = useCallback(
    async (next: number) => {
      if (!selected || !latest) return;
      const maxPage = Math.floor(latest.seq / PAGE);
      const clamped = Math.max(0, Math.min(next, maxPage));
      try {
        await loadPage(selected, latest.seq, clamped);
        setPage(clamped);
        setError(null);
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [selected, latest, loadPage],
  );

  const create = useCallback(
    async (s: SeriesSchema) => {
      // Persist the schema (a real record via the ingest path), refresh the list, then drop into it.
      await saveSchema(s);
      await search("");
      await select(s.series);
    },
    [search, select],
  );

  const write = useCallback(
    async (sample: Sample) => {
      try {
        await writeSample(sample);
        setError(null);
        // Refresh the list (a brand-new series should appear) and the selected detail.
        await search("");
        if (sample.series === selected) await select(sample.series);
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [search, select, selected],
  );

  // Initial series list.
  useEffect(() => {
    void search("");
  }, [search]);

  return {
    series,
    selected,
    schema,
    latest,
    recent,
    page,
    hasOlder,
    error,
    search,
    select,
    goToPage,
    write,
    create,
  };
}
