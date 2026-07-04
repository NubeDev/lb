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

/** The client-reported PAGE CONTEXT (agent-dock scope) — where the user is when they ask. The host
 *  fences it into the run's goal as untrusted context (rule 10: the surface id is opaque data, never
 *  branched on). Mirrors the Rust `AgentPayload.context` / `InvokeRequest.context` opaque `Value`. */
export interface PageContext {
  /** The Surface the user is on (from `surfaceForPath`) — an opaque string. */
  surface: string;
  /** The tenant-stripped pathname (no `/t/<ws>` prefix). */
  path: string;
  /** The typed search params, flat. */
  search: Record<string, string>;
}

/** `kind: "agent"` — a member's request to ask an agent `goal` (channels-agent scope). */
export interface AgentPayload {
  kind: "agent";
  goal: string;
  /** The runtime selector: absent → the in-house default; a profile id → an external agent. */
  runtime?: string;
  /** The durable run id (the UI mints it so it can subscribe to the run stream immediately). */
  job: string;
  /** Optional page context (agent-dock scope) — fenced into the run's goal as untrusted context.
   *  Absent → byte-identical to a plain channel agent post. */
  context?: PageContext;
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

/** `kind: "rich_result"` — the render-envelope (channel rich responses scope). Mirrors the Rust
 *  `RichResultPayload` (rust/crates/host/src/channel/payload.rs) ONE-TO-ONE. A worker's viewable
 *  response: a `view` over inline `data` and/or a re-runnable `source`, with row-control `options`, an
 *  optional control `action`, and the `tools` the response's bridge may forward. `v` is the envelope
 *  version and is ALWAYS on the wire — a reader keys any upconversion on it, and a `v` newer than the UI
 *  understands degrades AT RENDER (ResponseView), never at parse. */
export interface RichResultPayload {
  kind: "rich_result";
  /** The render-envelope version — always `2`. */
  v: 2;
  /** The viewer to render with (`table`/`chart`/`stat`/`switch`/`button`/`template`). */
  view: string;
  /** A `{tool, args}` object the viewer re-runs to (re)load data. Absent → the response is inline-only. */
  source?: { tool: string; args?: Record<string, unknown> };
  /** Inline data the viewer renders directly. Absent → the viewer runs `source`. */
  data?: unknown;
  /** View options (incl. row controls). Absent → the viewer's defaults. */
  options?: Record<string, unknown>;
  /** A control's `{tool, argsTemplate}` (a button/switch's effect). Absent → the view is read-only. */
  action?: { tool: string; argsTemplate?: Record<string, unknown> };
  /** The declared tool set the response's bridge may forward (the `source` + `action` + row-control
   *  tools). The host intersects it with the viewer's grant server-side (render.tools ∩ grant). */
  tools?: string[];
  /** The per-field PRESENTATION config for the rendered view (widget-kit scope, Phase 1) — the Grafana
   *  `fieldConfig` a descriptor declares so a table's headers read the author's labels (`displayName`),
   *  drop hidden columns (`hide`), and order as declared. INERT DATA that rides this existing envelope
   *  (no new verb/table) — `ResponseView.buildCell` copies it onto `cell.fieldConfig`, and the shared
   *  table column-model resolves every header through it. Absent → the table humanizes raw keys. */
  fieldConfig?: import("@/lib/dashboard").FieldConfig;
}

/** The kind-tagged union pulled out of an item `body`. Chat (no `kind`) is `null`. */
export type ItemPayload =
  | QueryPayload
  | QueryResultPayload
  | QueryErrorPayload
  | AgentPayload
  | AgentResultPayload
  | AgentErrorPayload
  | RichResultPayload;

const KINDS = new Set([
  "query",
  "query_result",
  "query_error",
  "agent",
  "agent_result",
  "agent_error",
  "rich_result",
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
export function encodeAgent(
  goal: string,
  job: string,
  runtime?: string,
  context?: PageContext,
): string {
  const payload: AgentPayload = { kind: "agent", goal, job };
  if (runtime) payload.runtime = runtime;
  // Page context rides on the payload only when captured (agent-dock); absent → byte-identical to a
  // plain channel agent post (the host fences it in; the UI is a thin client).
  if (context) payload.context = context;
  return JSON.stringify(payload);
}

/** Encode a `rich_result` render-envelope into the body string the channel `post` carries — the UI
 *  posts this to render a structured response (e.g. a `reminder.list` table). Stamps `kind` + `v:2`,
 *  mirroring `rich_result_body` on the Rust side. Callers pass the render fields; the version is always
 *  2 (additive/versioned — a reader degrades at render, not parse). */
export function encodeRichResult(
  payload: Omit<RichResultPayload, "kind" | "v"> & { v?: 2 },
): string {
  const body: RichResultPayload = { ...payload, kind: "rich_result", v: 2 };
  return JSON.stringify(body);
}
