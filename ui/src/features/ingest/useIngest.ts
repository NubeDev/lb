// The Ingest-page hook — data + state for the series explorer (data-console scope). Lists/searches
// the workspace's series, loads a selected series' latest + recent samples, and pushes one sample by
// hand. Snapshot reads + manual refresh (no live motion — that is the dashboard's job). One hook per
// file (FILE-LAYOUT). Everything runs against the real gateway; the producer is the token's principal.

import { useCallback, useEffect, useState } from "react";

import {
  findSeries,
  latestSample,
  listSeries,
  readSamples,
  writeSample,
} from "@/lib/ingest/ingest.api";
import type { Facet, Sample } from "@/lib/ingest/ingest.types";

/** How many recent samples the detail table reads (the tail). */
const RECENT = 50;

export interface IngestState {
  series: string[];
  selected: string | null;
  latest: Sample | null;
  recent: Sample[];
  error: string | null;
  /** Refresh the series list — `query` is a `key:value` facet filter, or a plain prefix, or empty. */
  search: (query: string) => Promise<void>;
  /** Select a series and load its latest + recent samples. */
  select: (series: string) => Promise<void>;
  /** Push one sample (the manual-write form), then refresh the selected series' table. */
  write: (sample: Sample) => Promise<void>;
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
  const [latest, setLatest] = useState<Sample | null>(null);
  const [recent, setRecent] = useState<Sample[]>([]);
  const [error, setError] = useState<string | null>(null);

  const search = useCallback(async (query: string) => {
    try {
      const facet = parseFacet(query);
      // A `key:value` query is tag-faceted discovery; a plain string is a prefix list.
      setSeries(facet ? await findSeries([facet]) : await listSeries(query.trim()));
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  const select = useCallback(async (s: string) => {
    setSelected(s);
    try {
      const [last, rows] = await Promise.all([latestSample(s), readSamples(s)]);
      setLatest(last);
      // Newest first (the read is seq-ascending; the table shows recent-first).
      setRecent(rows.slice(-RECENT).reverse());
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

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

  return { series, selected, latest, recent, error, search, select, write };
}
