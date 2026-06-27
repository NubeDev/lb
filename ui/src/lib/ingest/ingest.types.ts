// The ingest/series wire shapes — mirror the gateway's `ingest.*`/`series.*` routes (data-console
// scope). The Ingest page is the series explorer: list/search series, see latest + recent samples,
// and push one sample by hand. These are the S8 verbs, finally reachable over the gateway.

/** One sample of a series. The `producer` is the authenticated principal (the host overwrites any
 *  client-supplied value — un-spoofable). Mirrors `lb_ingest::Sample`. */
export interface Sample {
  series: string;
  producer: string;
  ts: number;
  seq: number;
  payload: unknown;
  labels?: Record<string, unknown>;
  qos?: "best-effort" | "must-deliver";
}

/** One facet of a `series.find` query: an exact `key=value`, or key-only when `value` is omitted. */
export interface Facet {
  key: string;
  value?: unknown;
}
