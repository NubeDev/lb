// The ingest/series API client — one call per export, mirroring the gateway's `ingest.*`/`series.*`
// routes and the host verbs 1:1 (data-console scope). The UI never calls `invoke` directly; it goes
// through these named verbs (FILE-LAYOUT frontend rules). Each is capability-gated server-side; the
// workspace + producer come from the session token (the hard wall, §7), never an argument — so a
// manually-written sample's producer is the authenticated principal.

import type { Facet, Sample } from "./ingest.types";
import { invoke } from "@/lib/ipc/invoke";

/** Push one sample by hand. The producer is set to the token's principal server-side; the workspace
 *  is drained so the sample is immediately visible to the reads. Mirrors `ingest.write`. */
export function writeSample(sample: Sample): Promise<{ accepted: number; committed: number }> {
  return invoke("ingest_write", { samples: [sample] });
}

/** List the workspace's series names, optionally by prefix (the discovery list). Mirrors
 *  `series.list`. */
export function listSeries(prefix?: string): Promise<string[]> {
  return invoke<{ series: string[] }>("series_list", { prefix }).then((r) => r.series);
}

/** Find series whose entity carries ALL `facets` (tag-graph intersection). Mirrors `series.find`. */
export function findSeries(facets: Facet[]): Promise<string[]> {
  return invoke<{ series: string[] }>("series_find", { facets }).then((r) => r.series);
}

/** The newest committed sample of `series` (or `null`). Mirrors `series.latest`. */
export function latestSample(series: string): Promise<Sample | null> {
  return invoke<{ sample: Sample | null }>("series_latest", { series }).then((r) => r.sample);
}

/** Read committed samples of `series` in `[from, to]` ordered by seq. Mirrors `series.read`. */
export function readSamples(series: string, from?: number, to?: number): Promise<Sample[]> {
  return invoke<{ samples: Sample[] }>("series_read", { series, from, to }).then((r) => r.samples);
}
