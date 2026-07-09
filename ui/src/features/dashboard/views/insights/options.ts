// The per-view `options` block for an `insights` cell (insights-package-scope). Mirrors the
// stat/gauge `options.ts` pattern: a typed shape, a fresh default, and a reader that coerces a
// persisted (possibly partial / legacy) `cell.options.insights` into a complete config the view +
// the `@nube/insights` widget consume. One responsibility: what a fresh insights cell's options are.

import type { ListQuery, Severity, Status } from "@nube/insights";

/** The insights cell's option block (persisted under `cell.options.insights`). Filters + presentation
 *  the dashboard author sets; the view folds them into the widget's `filter` + `interactive` props. */
export interface InsightsOptions {
  /** Read-only (default true) — no ack/resolve/dismiss buttons. Off ⇒ end users can triage in place. */
  readOnly: boolean;
  /** Status filter — `"all"` (default) drops the facet. */
  status: Status | "all";
  /** Severity filter — `"all"` (default) drops the facet. */
  severity: Severity | "all";
  /** Max rows fetched (the list's `limit`). */
  limit: number;
  /** Show the header refresh button (default true). */
  showRefresh: boolean;
}

/** A fresh insights cell's options — read-only, unfiltered, 20 rows. */
export function defaultInsightsOptions(): InsightsOptions {
  return { readOnly: true, status: "all", severity: "all", limit: 20, showRefresh: true };
}

/** Coerce a persisted (partial/legacy) options block into a complete {@link InsightsOptions}. Unknown
 *  values fall back to the default — a hand-built or older cell never crashes the view. */
export function readInsightsOptions(raw: unknown): InsightsOptions {
  const d = defaultInsightsOptions();
  const o = (raw ?? {}) as Record<string, unknown>;
  const status = o.status;
  const severity = o.severity;
  return {
    readOnly: typeof o.readOnly === "boolean" ? o.readOnly : d.readOnly,
    status:
      status === "open" || status === "acked" || status === "resolved" ? status : "all",
    severity:
      severity === "info" || severity === "warning" || severity === "critical"
        ? severity
        : "all",
    limit: typeof o.limit === "number" && o.limit > 0 ? Math.floor(o.limit) : d.limit,
    showRefresh: typeof o.showRefresh === "boolean" ? o.showRefresh : d.showRefresh,
  };
}

/** Fold the options into the `@nube/insights` list query (drops `all` facets). */
export function insightsFilter(opts: InsightsOptions): ListQuery {
  const q: ListQuery = { limit: opts.limit };
  if (opts.status !== "all") q.status = opts.status;
  if (opts.severity !== "all") q.severity = opts.severity;
  return q;
}
