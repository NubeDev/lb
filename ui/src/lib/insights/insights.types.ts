// The insights view/DTO types — mirror the Rust records in `lb_insights` one-to-one
// (insights umbrella scope + occurrences/subscriptions/notify sub-scopes). The cross-stack
// symmetry is the point: an `Insight` has the same field names in the tool, the DTO, and the
// client (FILE-LAYOUT §4 "DTOs follow the verbs they describe").

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
  seq: number;
  ts: number;
  severity: Severity;
  data?: Record<string, unknown> | unknown[];
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
