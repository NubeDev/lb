// The kind-tagged channel item payloads (channels-query-charts scope) — mirror the Rust
// `lb_host::channel::payload` + `::chart` shapes (rust/crates/host/src/channel/payload.rs,
// chart.rs) ONE-TO-ONE. A channel `Item`'s `body` is opaque text; these typed envelopes ride
// INSIDE `body` as JSON, keyed by `kind`. A body that isn't JSON, or JSON with no recognized
// `kind`, is an ordinary chat message — so this is purely additive (untagged stays chat).
//
//   - `query`        — `{ kind, source, sql }`, posted by a member who wants to run a query.
//   - `query_result` — `{ kind, source, sql, columns, rows, chart?, truncated? }`, by the worker.
//   - `query_error`  — `{ kind, source, sql, error }`, by the worker on failure.
//   - `agent`        — `{ kind, goal, runtime?, job }`, posted by a member who wants to ask an agent
//                      (channels-agent scope). `runtime` selects the AgentRuntime (absent → in-house
//                      default; a profile id like `open-interpreter-default` → an external agent).
//                      `job` is the durable run id the UI mints so it can watch the run stream.
//   - `agent_result` — `{ kind, goal, runtime, job, answer, truncated? }`, by the agent worker.
//   - `agent_error`  — `{ kind, goal, error }`, by the agent worker (opaque on deny/unknown-runtime).

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

/** `kind: "agent"` — a member's request to ask an agent `goal` (channels-agent scope). */
export interface AgentPayload {
  kind: "agent";
  goal: string;
  /** The runtime selector: absent → the in-house default; a profile id → an external agent. */
  runtime?: string;
  /** The durable run id (the UI mints it so it can subscribe to the run stream immediately). */
  job: string;
}

/** `kind: "agent_result"` — the agent worker's durable final answer. */
export interface AgentResultPayload {
  kind: "agent_result";
  goal: string;
  /** The runtime that served the run (`"default"` or a profile id). */
  runtime: string;
  job: string;
  answer: string;
  /** True when the answer hit the byte cap and was trimmed. */
  truncated?: boolean;
}

/** `kind: "agent_error"` — the worker's opaque/honest failure (opaque on deny/unknown-runtime). */
export interface AgentErrorPayload {
  kind: "agent_error";
  goal: string;
  error: string;
}

/** The kind-tagged union pulled out of an item `body`. Chat (no `kind`) is `null`. */
export type ItemPayload =
  | QueryPayload
  | QueryResultPayload
  | QueryErrorPayload
  | AgentPayload
  | AgentResultPayload
  | AgentErrorPayload;

const KINDS = new Set([
  "query",
  "query_result",
  "query_error",
  "agent",
  "agent_result",
  "agent_error",
]);

/** Detect whether a `query_result` carries POSITIONAL array rows (one value per column, in
 *  `columns` order — the compact shape the worker PERSISTS) and zip them into the keyed objects the
 *  renderers expect. Rows that are already keyed objects pass through untouched (so a hand-seeded
 *  `query_result` with object rows still works). */
function normalizeResultRows(payload: QueryResultPayload): void {
  if (payload.rows.length === 0) return;
  if (!Array.isArray(payload.rows[0])) return;
  payload.rows = (payload.rows as unknown as unknown[][]).map((row) => {
    const obj: Record<string, unknown> = {};
    payload.columns.forEach((c, i) => {
      obj[c] = row[i];
    });
    return obj;
  });
}

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
  const payload = value as ItemPayload;
  if (payload.kind === "query_result") normalizeResultRows(payload);
  return payload;
}

/** Encode a `query` request into the body string the channel `post` carries. The UI builds this —
 *  the host never parses chat text into a command. */
export function encodeQuery(source: string, sql: string): string {
  const payload: QueryPayload = { kind: "query", source, sql };
  return JSON.stringify(payload);
}

/** Mint a run id for an `agent` request. Kept tiny + injectable-free; uniqueness only needs to hold
 *  within a channel's lifetime (it keys the run stream + correlates the `agent_result`). */
export function newRunId(): string {
  const rand = Math.random().toString(36).slice(2, 10);
  return `run-${Date.now().toString(36)}-${rand}`;
}

/** Encode an `agent` request body (channels-agent scope). `runtime` omitted → the in-house default;
 *  pass a profile id (e.g. `open-interpreter-default`) to drive an external agent. The UI mints `job`
 *  (via {@link newRunId}) so it can watch the run stream the instant the request lands. */
export function encodeAgent(goal: string, job: string, runtime?: string): string {
  const payload: AgentPayload = { kind: "agent", goal, job };
  if (runtime) payload.runtime = runtime;
  return JSON.stringify(payload);
}
