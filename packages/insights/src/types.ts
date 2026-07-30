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

/** One plottable series the finding sits on. Mirrors `lb_insights::EvidenceSeries`. */
export interface EvidenceSeries {
  /** A query yielding `(time, value)` rows. Dialect is the datasource's business. */
  sql: string;
  label?: string;
  unit?: string;
}

/** The data that proves a finding — the producer's own binding. Mirrors `lb_insights::Evidence`
 *  (`docs/scope/insights/insight-evidence-scope.md`).
 *
 *  `series` is NOT the rule's judgment query: a rule that judges with a `GROUP BY` aggregate has no
 *  time axis to plot, so it states the underlying per-entity series separately. Draw `series`; treat
 *  `query` as provenance only. A reader turns each series into one panel target —
 *  `{tool: evidence.tool ?? "federation.query", args: {source, sql}}`. */
export interface Evidence {
  /** Datasource id the series resolve against, resolved by the reader per-workspace. */
  source: string;
  series?: EvidenceSeries[];
  /** The judgment query — provenance/"open evidence" only, frequently not plottable. */
  query?: string;
  /** The window judged, epoch-ms — lets a viewer open pre-ranged. */
  window?: { from: number; to: number };
  /** The threshold crossed, in the series' own units — draw as a threshold line. */
  threshold?: number;
  /** Data-plane verb the series dispatch through; absent ⇒ `"federation.query"`. */
  tool?: string;
}

/** A measured quantity on an {@link Analysis} — a number + unit, a note, or both. Mirrors
 *  `lb_insights::Quantity`.
 *
 *  Typed rather than prose so findings can be RANKED ("today's findings by cost"). Three legal
 *  shapes: note-only, `value` + `unit`, or all three — the server refuses a `value` with no `unit`
 *  and an all-absent object, so any quantity you receive has at least one of them.
 *
 *  Rendering: prefer `note`, else format `value` + `unit`. Sorting: read `value` and skip rows
 *  without one. Aggregating across producers: **group by `unit`** and refuse to sum unlike units —
 *  `unit` is free text, so nothing stops one rule writing `"AUD/day"` and another `"$/day"`. */
export interface Quantity {
  value?: number;
  /** Unit of `value` — `"%"`, `"kL"`, `"AUD/day"`. Always present when `value` is. */
  unit?: string;
  /** The producer's own words — the honest `"N/A (data quality)"`, or context beside a number. */
  note?: string;
}

/** The producer's own REASONING about a finding. Mirrors `lb_insights::Analysis`
 *  (`docs/scope/insights/insight-analysis-scope.md`).
 *
 *  `Evidence` says where the data is; this says what the producer concluded. Render as a fixed
 *  labelled layout — the value of these six names is that every consumer shows the same ones in the
 *  same order. **The struct is CLOSED server-side**: a seventh key is dropped silently, so anything
 *  else lives in `body`.
 *
 *  Nothing here is computed or verified by the node — these are the producer's claims, stored
 *  verbatim. Treat `suspected_cause` as a hypothesis and `estimated_impact` as an unfalsifiable
 *  estimate with no provenance on the record; attribute both, never assert them. */
export interface Analysis {
  /** Why it fired, in the producer's words. */
  trigger_logic?: string;
  /** The producer's HYPOTHESIS — never a diagnosis. Label it "Suspected cause": the hedge lives in
   *  the field name *and* the label, and this is the one place it can quietly evaporate. */
  suspected_cause?: string;
  /** The metric judged, normalised. */
  normalised_metric?: string;
  /** What it was compared against. */
  benchmark_context?: string;
  /** How far off. */
  deviation?: Quantity;
  /** Consequence if unaddressed — the field reports rank by. */
  estimated_impact?: Quantity;
}

/** One durable insight record. Mirrors `lb_insights::Insight`. */
export interface Insight {
  id: string;
  dedup_key: string;
  severity: Severity;
  title: string;
  body?: Record<string, unknown> | unknown[];
  /** The data that proves this finding. Echoed by `insight.get`; **absent on `insight.list` rows**
   *  (the roster omits it — page bloat + schema disclosure), so a list-driven view must `get` the
   *  record before it can bind a trend. Also absent on any record whose producer stated none. */
  evidence?: Evidence;
  /** The producer's reasoning. Echoed by `insight.get`; **absent on `insight.list` rows** — the same
   *  boundary `evidence` holds, so a roster must `get` the record before a drawer can render it.
   *  Also absent on any record whose producer stated none.
   *
   *  Note the contrast with `tags` below: `analysis` refreshes on every raise that supplies it, so
   *  it describes the LATEST firing — while `title`/`body` are still first-raise-wins, meaning a
   *  long-lived finding can show fresh reasoning above a firing-#1 narrative
   *  (`insight-prose-refresh-scope.md`). Don't present the two as one coherent snapshot. */
  analysis?: Analysis;
  origin: Origin;
  status: Status;
  status_by?: string;
  status_ts?: number;
  /** Who OWNS this finding — the human triage axis (`insight-triage-scope.md`). A **subject, not a
   *  user id**: `user:priya` and `team:mechanical` are both legal, so never assume a `user:` prefix
   *  when rendering. Absent = unassigned (filter `assigned_to: "none"` for the triage queue).
   *
   *  Present on BOTH `insight.get` and `insight.list` rows — the tag-echo boundary, not the
   *  `evidence` one: this is the owner COLUMN, so a roster renders it with no N+1. Its sibling
   *  `comments` takes the opposite side of that boundary.
   *
   *  **Survives re-raise, including re-open.** Unlike `status_by`/`status_ts` — which clear when a
   *  resolved finding fires again — this is a human fact and is never touched by the producer path.
   *  A UI must not imply assigning NOTIFIED anyone: v1 assignment is a roster fact only.
   *
   *  A subject outlives its membership. Render an assignee you cannot resolve as
   *  **"unknown (removed)"**, never blank — a silently empty owner column hides an orphaned queue. */
  assigned_to?: string;
  /** The full triage thread, newest-first. Present on `insight.get` only — **never on
   *  `insight.list` rows**, so a roster must `get` before a drawer can render it (the `evidence`
   *  boundary; the thread is the payload most able to make every page expensive).
   *
   *  Complete, not a window: comments don't evict, so there is no cursor and no "load older". */
  comments?: Comment[];
  count: number;
  first_ts: number;
  last_ts: number;
  producer: string;
  /** The insight's tag facets, **echoed** — the dimension plane (building, asset type, priority, …)
   *  as a flat map, present on BOTH `insight.get` and `insight.list` rows. The deliberate
   *  divergence from `evidence`: dimensions exist to be roster COLUMNS, so a list-driven view
   *  renders them with no follow-up `tags.find` per row and needs no tag capability.
   *
   *  Read-only: the tag graph is the source of truth and the raise path writes this projection.
   *  Never send it back, and never filter on it client-side across pages — `ListFilter.tags`
   *  resolves through the graph server-side and is correct even while an echo is briefly behind.
   *  Empty/absent on records raised before the field landed, until their next raise. */
  tags?: Record<string, string>;
}

/** One human note on an insight's append-only triage thread. Mirrors `lb_insights::Comment`
 *  (`docs/scope/insights/insight-triage-scope.md`).
 *
 *  **Append-only and never evicted.** Unlike the occurrence ring this borrows its storage shape
 *  from, comments are retained for the LIFE of the insight and purged only with it — so a thread you
 *  read is COMPLETE, not a recent window, and `cseq` is a permanent handle. v1 has no edit or
 *  delete: a correction is another comment, which is why the thread reads as what was actually known
 *  when.
 *
 *  Bounded by two REFUSALS, never a silent drop: an oversize `text` rejects the call, and appending
 *  past the per-insight count cap errors with the existing thread untouched. Surface both to the
 *  author — a note that looks saved but wasn't is the failure this design exists to prevent. */
export interface Comment {
  /** Monotone per-insight sequence (serialized as `cseq`). Stable — nothing evicts or deletes. */
  cseq: number;
  text: string;
  /** Always the principal's `sub`, host-stamped. Anything you send here is ignored. */
  author: string;
  ts: number;
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
  /** `assign`/`comment` ride this SAME feed rather than a triage-specific one — one stream per
   *  surface, not per feature. Both are lite: re-read the record to get the new owner or note. */
  kind: "raise" | "ack" | "resolve" | "assign" | "comment";
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
  /** Filter by OWNER. A subject (`user:priya` / `team:mechanical`), or one of two literals the
   *  server resolves: `"me"` — the caller **and every team they are on** (so team-assigned findings
   *  appear in "mine"; don't reimplement this client-side as a sub-equality check) — and `"none"`,
   *  the unassigned triage queue. Composes with every other axis and with keyset paging. */
  assigned_to?: string;
}

/** A subscription's AND-composed filter. Mirrors `lb_insights::SubFilter`. */
export interface SubFilter {
  origin_ref?: string;
  dedup_key?: string;
  tags?: Record<string, string>;
  severity_min?: Severity;
  /** Filter by OWNER — a subject (`user:priya` / `team:mechanical`) or `"me"`, which the server
   *  resolves to the **subscription owner and every team they are on** (not the caller — a sub fires
   *  without one), re-resolved at every fire so team changes take effect.
   *
   *  Doing two jobs, and the second is the important one:
   *    - **at raise time** — an ordinary AND axis: "notify us when a finding our crew owns fires";
   *    - **at assign time** — the **opt-in** for assignment notifications. A subscription without
   *      this field never receives an assignment event, which is what kept the feature additive.
   *
   *  So: assigning work notifies nobody unless such a subscription exists. Don't tell a user their
   *  assignee was informed without checking that one does. */
  assignee?: string;
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
  /** Assign / re-assign / un-assign (`assignee: null` clears). Needs `mcp:insight.assign:call` —
   *  distinct from `ack`/`resolve`, so a caller may hold one and not the other; gate the UI on the
   *  cap rather than assuming an acker can assign. The server validates the assignee is a member or
   *  team of this workspace and refuses opaquely if not. */
  assign(id: string, assignee: string | null): Promise<void>;
  /** Bulk assign, max 100 ids, with PER-ITEM results. Over the cap is an explicit error, never a
   *  silent truncation. **Surface the failures** — a green toast over 12 failed rows is the
   *  no-silent-caps rule broken at the last mile. */
  assignMany?(
    ids: string[],
    assignee: string | null,
  ): Promise<{ results: { id: string; ok: boolean; error?: string }[] }>;
  /** Append a note. Needs `mcp:insight.comment:call`. The author is host-stamped; `text` is capped
   *  and the call REFUSES rather than truncating, so show the error to whoever typed it. */
  comment(id: string, text: string): Promise<void>;
  occurrences(insightId: string, cursor?: OccCursor, limit?: number): Promise<OccurrencePage>;
  /** Optional live tail — `onEvent` per raise/ack/resolve; returns an unsubscribe. Absent → no feed. */
  subscribe?(onEvent: (event: InsightEvent) => void): () => void;
}
