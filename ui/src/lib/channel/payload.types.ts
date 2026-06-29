// The kind-tagged channel item payloads (channels-query-charts scope) — mirror the Rust
// `lb_host::channel::payload` + `::chart` shapes (rust/crates/host/src/channel/payload.rs,
// chart.rs) ONE-TO-ONE. A channel `Item`'s `body` is opaque text; these typed envelopes ride
// INSIDE `body` as JSON, keyed by `kind`. A body that isn't JSON, or JSON with no recognized
// `kind`, is an ordinary chat message — so this is purely additive (untagged stays chat).
//
//   - `query`        — `{ kind, source, sql }`, posted by a member who wants to run a query.
//   - `query_result` — `{ kind, source, sql, columns, rows, chart?, truncated? }`, by the worker.
//   - `query_error`  — `{ kind, source, sql, error }`, by the worker on failure.

/** The chart kinds the host picker emits (rendered verbatim as the `type` field). */
export type ChartType = "line" | "bar" | "histogram";

/** One chart series — a numeric column plotted against the x axis. Mirrors `ChartSeries`. */
export interface ChartSeries {
  field: string;
}

/** The chart spec embedded in a `query_result` payload. Mirrors the Rust `ChartSpec` (note the
 *  `type` rename and the histogram-only `bins`). The host computes this so EVERY subscriber renders
 *  the same chart — the UI renders it verbatim, it never re-derives. */
export interface ChartSpec {
  type: ChartType;
  x: string;
  series: ChartSeries[];
  /** Suggested bucket count — present only for a histogram. */
  bins?: number;
}

/** `kind: "query"` — a member's request to run `sql` against `source`. */
export interface QueryPayload {
  kind: "query";
  source: string;
  sql: string;
}

/** `kind: "query_result"` — the worker's answer: columns/rows (capped) + the host-picked chart. */
export interface QueryResultPayload {
  kind: "query_result";
  source: string;
  sql: string;
  columns: string[];
  /** Rows as JSON objects keyed by column (the federation frame shape). */
  rows: Record<string, unknown>[];
  /** The auto-plotted chart, or null/absent when nothing was safely plottable (table-only). */
  chart?: ChartSpec | null;
  /** True when the row/byte cap trimmed the result; the UI shows "showing first N rows". */
  truncated?: boolean;
}

/** `kind: "query_error"` — the worker's opaque/honest failure message. */
export interface QueryErrorPayload {
  kind: "query_error";
  source: string;
  sql: string;
  error: string;
}

/** The kind-tagged union pulled out of an item `body`. Chat (no `kind`) is `null`. */
export type ItemPayload = QueryPayload | QueryResultPayload | QueryErrorPayload;

const KINDS = new Set(["query", "query_result", "query_error"]);

/** Parse an item `body` into a kind-tagged payload, or `null` if it is chat (not JSON, or JSON
 *  without a recognized `kind`). Tolerant by design — mirrors the host's `parse_payload`. */
export function parsePayload(body: string): ItemPayload | null {
  let value: unknown;
  try {
    value = JSON.parse(body);
  } catch {
    return null;
  }
  if (typeof value !== "object" || value === null) return null;
  const kind = (value as { kind?: unknown }).kind;
  if (typeof kind !== "string" || !KINDS.has(kind)) return null;
  return value as ItemPayload;
}

/** Encode a `query` request into the body string the channel `post` carries. The UI builds this —
 *  the host never parses chat text into a command. */
export function encodeQuery(source: string, sql: string): string {
  const payload: QueryPayload = { kind: "query", source, sql };
  return JSON.stringify(payload);
}
