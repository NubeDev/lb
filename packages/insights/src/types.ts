// The canonical insights vocabulary + the injected transport seam.
//
// This package is TRANSPORT-AGNOSTIC by design (source-picker's discipline): it never imports an API
// client, `invoke`/`bridge`, or `@/`. The host supplies an `InsightsClient` — a bag of read/act
// functions — so ONE implementation works from the shell (gateway/Tauri), from a dashboard widget,
// and from a standalone extension UI (its host bridge) alike.
//
// The record shapes MIRROR the node's wire records one-to-one (the same field names the `insight.*`
// MCP verbs return — `lb_insights::Insight` etc.). They live here so the package stands alone; the
// shell's `@/lib/insights/*` types re-export / structurally match these (one shape, not two).

export type Severity = "info" | "warning" | "critical";
export type Status = "open" | "acked" | "resolved";
export type OriginKind = "rule" | "flow" | "agent" | "ext" | "manual";

/** Producer provenance — what raised it, from which run (`ref` is opaque to the host). */
export interface Origin {
  kind: OriginKind;
  ref: string;
  run?: string;
}

/** One durable insight record. Mirrors `lb_insights::Insight`. */
export interface Insight {
  id: string;
  dedup_key: string;
  severity: Severity;
  title: string;
  body?: Record<string, unknown> | unknown[];
  origin: Origin;
  status: Status;
  status_by?: string;
  status_ts?: number;
  count: number;
  first_ts: number;
  last_ts: number;
  producer: string;
}

/** One firing in the per-insight occurrence ring. Mirrors `lb_insights::Occurrence`. */
export interface Occurrence {
  oseq: number;
  ts: number;
  severity: Severity;
  data?: Record<string, unknown> | unknown[];
}

/** A live insight event on the `insight.watch` feed. Mirrors `lb_insights::RaiseEvent`. */
export interface InsightEvent {
  kind: "raise" | "ack" | "resolve";
  id: string;
  dedup_key: string;
  status: Status;
  severity: Severity;
  count: number;
  ts: number;
}

/** Keyset cursor — opaque to the caller; the verb parses it. */
export interface PageCursor {
  ts: number;
  id: string;
}

/** The AND-composed list filter. Mirrors `lb_insights::ListFilter`. */
export interface ListFilter {
  status?: Status;
  severity?: Severity;
  origin_ref?: string;
  tags?: Record<string, string>;
  range?: [number, number];
}

/** The full list query (filter + paging + limit). Mirrors `lb_insights::ListQuery`. */
export interface ListQuery extends ListFilter {
  cursor?: PageCursor;
  limit?: number;
}

/** One newest-first page of insights. Mirrors `lb_insights::ListPage`. */
export interface ListPage {
  items: Insight[];
  next?: PageCursor;
}

/** The occurrence-ring cursor. Mirrors `lb_insights::OccCursor`. */
export interface OccCursor {
  seq: number;
}

/** One newest-first page of the occurrence ring. Mirrors `lb_insights::OccurrencePage`. */
export interface OccurrencePage {
  items: Occurrence[];
  next?: OccCursor;
}

/** The injected transport seam — how a host reaches the node's `insight.*` verbs. Every method maps
 *  1:1 to a verb; the host implements them over its own transport (the shell's `/mcp/call` bridge, an
 *  extension's host bridge). A read the caller isn't granted may reject — the hooks surface that as an
 *  error, never a fabricated list (CLAUDE §9). `subscribe` is OPTIONAL: a host with no live feed (the
 *  Tauri shell, tests) omits it and the hooks fall back to the act→refresh round trip.
 *
 *  `ack`/`resolve` take no timestamp: the host stamps `ts: Date.now()` at the transport (the package
 *  is pure and can't call `Date.now()` deterministically — see the shell's `insights.api.ts`). */
export interface InsightsClient {
  list(query: ListQuery): Promise<ListPage>;
  get(id: string): Promise<Insight | null>;
  ack(id: string): Promise<void>;
  resolve(id: string, note?: string): Promise<void>;
  occurrences(insightId: string, cursor?: OccCursor, limit?: number): Promise<OccurrencePage>;
  /** Optional live tail — `onEvent` per raise/ack/resolve; returns an unsubscribe. Absent → no feed. */
  subscribe?(onEvent: (event: InsightEvent) => void): () => void;
}
