import { JSX as JSX_2 } from 'react';
import { ReactNode } from 'react';

/** A client whose reads reject — models a workspace granted no `insight.list` cap. The hooks must
 *  surface this as an honest error, never a fabricated list. */
export declare function denyClient(): InsightsClient;

/** The data that proves a finding — the producer's own binding. Mirrors `lb_insights::Evidence`
 *  (`docs/scope/insights/insight-evidence-scope.md`).
 *
 *  `series` is NOT the rule's judgment query: a rule that judges with a `GROUP BY` aggregate has no
 *  time axis to plot, so it states the underlying per-entity series separately. Draw `series`; treat
 *  `query` as provenance only. A reader turns each series into one panel target —
 *  `{tool: evidence.tool ?? "federation.query", args: {source, sql}}`. */
export declare interface Evidence {
    /** Datasource id the series resolve against, resolved by the reader per-workspace. */
    source: string;
    series?: EvidenceSeries[];
    /** The judgment query — provenance/"open evidence" only, frequently not plottable. */
    query?: string;
    /** The window judged, epoch-ms — lets a viewer open pre-ranged. */
    window?: {
        from: number;
        to: number;
    };
    /** The threshold crossed, in the series' own units — draw as a threshold line. */
    threshold?: number;
    /** Data-plane verb the series dispatch through; absent ⇒ `"federation.query"`. */
    tool?: string;
}

/** One plottable series the finding sits on. Mirrors `lb_insights::EvidenceSeries`. */
export declare interface EvidenceSeries {
    /** A query yielding `(time, value)` rows. Dialect is the datasource's business. */
    sql: string;
    label?: string;
    unit?: string;
}

/** One durable insight record. Mirrors `lb_insights::Insight`. */
export declare interface Insight {
    id: string;
    dedup_key: string;
    severity: Severity;
    title: string;
    body?: Record<string, unknown> | unknown[];
    /** The data that proves this finding. Echoed by `insight.get`; **absent on `insight.list` rows**
     *  (the roster omits it — page bloat + schema disclosure), so a list-driven view must `get` the
     *  record before it can bind a trend. Also absent on any record whose producer stated none. */
    evidence?: Evidence;
    origin: Origin;
    status: Status;
    status_by?: string;
    status_ts?: number;
    count: number;
    first_ts: number;
    last_ts: number;
    producer: string;
}

/** The ack/resolve/dismiss button row. Renders only the actions the current status allows. */
export declare function InsightActions({ insight, actingOn, onAck, onResolve, onDismiss, }: InsightActionsProps): JSX_2.Element;

export declare interface InsightActionsProps {
    insight: Insight;
    /** The in-flight action (drives the spinner + disable), or null. */
    actingOn?: "ack" | "resolve" | null;
    onAck?: () => void;
    onResolve?: () => void;
    /** Optional local dismiss (hide the row) — distinct from `resolve` (a durable status change). */
    onDismiss?: () => void;
}

export declare interface InsightDetailState {
    insight: Insight | null;
    occurrences: OccurrencePage | null;
    error: string | null;
    loading: boolean;
    /** Ack/resolve-in-flight action, or null when idle. */
    actingOn: "ack" | "resolve" | null;
    refresh: () => void;
    act: (action: "ack" | "resolve") => Promise<void>;
}

/** A live insight event on the `insight.watch` feed. Mirrors `lb_insights::RaiseEvent`. */
export declare interface InsightEvent {
    kind: "raise" | "ack" | "resolve";
    id: string;
    dedup_key: string;
    status: Status;
    severity: Severity;
    count: number;
    ts: number;
}

/** Render one insight row. */
export declare function InsightRow({ insight, selected, onSelect, showStatus, showSeverity, actions, now, }: InsightRowProps): JSX_2.Element;

export declare interface InsightRowProps {
    insight: Insight;
    selected?: boolean;
    /** Click handler — present → the row is a button; absent → a static row (read-only). */
    onSelect?: (id: string) => void;
    /** Which badges to show on the side column. Status shows by default; severity is already carried by
     *  the leading dot, so its redundant chip is off by default (opt in for a legend-style row). */
    showStatus?: boolean;
    showSeverity?: boolean;
    /** Optional inline actions node (rendered below the row body) — the acknowledge widget's buttons. */
    actions?: ReactNode;
    /** `now` for the time-ago (test determinism). */
    now?: number;
}

/** Acknowledge preset — each row carries ack / resolve / dismiss. */
export declare function InsightsAckWidget(props: Omit<InsightsWidgetProps, "interactive">): JSX_2.Element;

/** The injected transport seam — how a host reaches the node's `insight.*` verbs. Every method maps
 *  1:1 to a verb; the host implements them over its own transport (the shell's `/mcp/call` bridge, an
 *  extension's host bridge). A read the caller isn't granted may reject — the hooks surface that as an
 *  error, never a fabricated list (CLAUDE §9). `subscribe` is OPTIONAL: a host with no live feed (the
 *  Tauri shell, tests) omits it and the hooks fall back to the act→refresh round trip.
 *
 *  `ack`/`resolve` take no timestamp: the host stamps `ts: Date.now()` at the transport (the package
 *  is pure and can't call `Date.now()` deterministically — see the shell's `insights.api.ts`). */
export declare interface InsightsClient {
    list(query: ListQuery): Promise<ListPage>;
    get(id: string): Promise<Insight | null>;
    ack(id: string): Promise<void>;
    resolve(id: string, note?: string): Promise<void>;
    occurrences(insightId: string, cursor?: OccCursor, limit?: number): Promise<OccurrencePage>;
    /** Optional live tail — `onEvent` per raise/ack/resolve; returns an unsubscribe. Absent → no feed. */
    subscribe?(onEvent: (event: InsightEvent) => void): () => void;
}

/** Read-only preset — a glanceable list, no actions. */
export declare function InsightsReadWidget(props: Omit<InsightsWidgetProps, "interactive">): JSX_2.Element;

export declare interface InsightsState {
    items: Insight[];
    error: string | null;
    loading: boolean;
    /** Ack/resolve-in-flight item id, or null when idle (per-row disable + spin, the inbox pattern). */
    actingOn: string | null;
    /** The keyset cursor for the next page, or null when the current list is the last page. */
    nextCursor: PageCursor | null;
    refresh: () => Promise<void>;
    loadMore: () => Promise<void>;
    setFilter: (filter: ListQuery) => void;
    act: (id: string, action: "ack" | "resolve") => Promise<void>;
}

/** The insights widget. Read-only or acknowledge, one component. */
export declare function InsightsWidget({ client, filter, title, interactive, showRefresh, paged, onSelect, now, }: InsightsWidgetProps): JSX_2.Element;

export declare interface InsightsWidgetProps {
    /** The injected transport seam (how to reach the node's `insight.*` verbs). */
    client: InsightsClient;
    /** The starting filter (status / severity / tags / range / limit). Defaults to `{ limit: 20 }`. */
    filter?: ListQuery;
    /** Panel title. Defaults to "Insights". */
    title?: string;
    /** When true, each row carries ack / resolve / dismiss actions; when false (default), read-only. */
    interactive?: boolean;
    /** Show the header refresh button. Default: true. */
    showRefresh?: boolean;
    /** Show the "Load more" footer when a next page exists. Default: true. */
    paged?: boolean;
    /** Click a row (e.g. to open a host detail surface). Rows are static when omitted. */
    onSelect?: (id: string) => void;
    /** `now` for the time-ago (test determinism). */
    now?: number;
}

/** The AND-composed list filter. Mirrors `lb_insights::ListFilter`. */
export declare interface ListFilter {
    status?: Status;
    severity?: Severity;
    origin_ref?: string;
    tags?: Record<string, string>;
    range?: [number, number];
}

/** One newest-first page of insights. Mirrors `lb_insights::ListPage`. */
export declare interface ListPage {
    items: Insight[];
    next?: PageCursor;
}

/** The full list query (filter + paging + limit). Mirrors `lb_insights::ListQuery`. */
export declare interface ListQuery extends ListFilter {
    cursor?: PageCursor;
    limit?: number;
}

/** Build a real in-memory client over a seeded set of insights (newest-first by `last_ts`). */
export declare function memoryClient(seed: Insight[]): InsightsClient;

/** The occurrence-ring cursor. Mirrors `lb_insights::OccCursor`. */
export declare interface OccCursor {
    seq: number;
}

/** One firing in the per-insight occurrence ring. Mirrors `lb_insights::Occurrence`. */
export declare interface Occurrence {
    oseq: number;
    ts: number;
    severity: Severity;
    data?: Record<string, unknown> | unknown[];
}

/** One newest-first page of the occurrence ring. Mirrors `lb_insights::OccurrencePage`. */
export declare interface OccurrencePage {
    items: Occurrence[];
    next?: OccCursor;
}

/** Producer provenance — what raised it, from which run (`ref` is opaque to the host). */
export declare interface Origin {
    kind: OriginKind;
    ref: string;
    run?: string;
}

export declare type OriginKind = "rule" | "flow" | "agent" | "ext" | "manual";

/** The producer/run meta line under a title ("rule:cpu-hot · run:abc"). Pure — the UI + a host reuse it. */
export declare function originLine(origin: {
    kind: string;
    ref: string;
    run?: string;
}): string;

/** Keyset cursor — opaque to the caller; the verb parses it. */
export declare interface PageCursor {
    ts: number;
    id: string;
}

export declare type Severity = "info" | "warning" | "critical";

/** Severity floor ordering (info < warning < critical) — a `severity` filter is a FLOOR: selecting
 *  `warning` means warning-and-above. The index is the numeric rank for comparisons. */
export declare const SEVERITY_ORDER: Severity[];

/** A severity chip ("CRITICAL" etc.), tinted by the tone key. */
export declare function SeverityBadge({ severity }: {
    severity: Severity;
}): JSX_2.Element;

/** Numeric rank of a severity (info=0 … critical=2). */
export declare function severityRank(s: Severity): number;

/** Severity → tone key. */
export declare function severityTone(s: Severity): Tone;

export declare type Status = "open" | "acked" | "resolved";

/** A status chip ("OPEN" / "ACKED" / "RESOLVED"), tinted by the tone key. */
export declare function StatusBadge({ status }: {
    status: Status;
}): JSX_2.Element;

/** Status → tone key. `open` reads as the primary accent (action due), `acked` as warning (claimed),
 *  `resolved` as success (done) — the Inbox status register. */
export declare function statusTone(s: Status): Tone;

/** A compact relative-time formatter ("2m ago", "1h 22m ago", "3d ago"). `now` defaults to the wall
 *  clock; pass it explicitly for a deterministic test (the package itself never calls `Date.now()`
 *  in a way that leaks into a snapshot). */
export declare function timeAgo(ts: number, now?: number): string;

/** A tone KEY per severity — a stable, look-free token a host maps to its own palette. The package UI
 *  maps `critical → destructive`, `warning → warning`, `info → accent-2`; a host may map differently. */
export declare type Tone = "destructive" | "warning" | "accent-2" | "default" | "success";

/** Load + drive the detail for insight `id` over `client`. Re-fetches on `id` change and after an
 *  ack/resolve lands (so the pane re-opens with the new status). `occLimit` bounds the occurrence page
 *  (default 50). `client` is read through a ref (host-stability — see `useInsights`). */
export declare function useInsight(client: InsightsClient, id: string, occLimit?: number): InsightDetailState;

/** Drive an insights list over `client`. `initial` is the starting filter (status / severity / tags /
 *  range); `setFilter` swaps it. Keyset paging appends on `loadMore`; the client's `subscribe` feed (if
 *  any) refreshes the head on each raise/ack/resolve. `client` is read through a ref so an unmemoized
 *  literal per render does not loop (the source-picker host-stability guarantee). */
export declare function useInsights(client: InsightsClient, initial: ListQuery): InsightsState;

export { }
