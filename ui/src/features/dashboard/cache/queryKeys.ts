// Canonical react-query keys for the dashboard read cache (dashboard-query-cache-scope). "Query-key design
// is the whole ballgame" (scope, Risks): too coarse → spurious refetches (today's whole-panel-JSON key);
// too unstable (object identity, member reordering, `undefined` leaves) → cache misses that look like the
// old behaviour. Every key here is:
//   1. **ws-prefixed** — a workspace switch changes the key → different cache entries, no cross-ws bleed
//      (the host still re-checks the ws from the token regardless; the key is de-dup, not the security wall).
//   2. **canonicalised** — objects go through `canon()` (sorted keys, dropped `undefined`) so an unrelated
//      edit (member order, a title change that never reaches these fields) does NOT re-key.
// The token is NEVER part of a key (it lives in the shell/gateway seam; the cache never sees it).

/** Deterministically canonicalise a value: object keys sorted, `undefined` members dropped, arrays kept in
 *  order (order is meaningful for targets/paths). The result is stable across unrelated identity churn, so
 *  two structurally-equal specs hash to the SAME key. */
export function canon(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(canon);
  if (value && typeof value === "object") {
    const out: Record<string, unknown> = {};
    for (const k of Object.keys(value as Record<string, unknown>).sort()) {
      const v = (value as Record<string, unknown>)[k];
      if (v !== undefined) out[k] = canon(v);
    }
    return out;
  }
  return value;
}

/** The resolved viz.query spec that actually drives the fetch — NOT the whole panel. Title/layout/option
 *  edits are absent here, so they don't re-key (scope goal 2). `tick` folds the refresh cadence into the
 *  key so a new tick is a new entry ("fresh until next tick"). */
export interface VizQuerySpec {
  sources: unknown;
  transformations: unknown;
  fieldConfig: unknown;
  source: unknown;
  scope: unknown;
  tick: number;
}

/** `viz.query` — keyed on the canonical resolved spec + scope + tick, ws-prefixed. */
export function vizQueryKey(ws: string, spec: VizQuerySpec) {
  return ["viz.query", ws, canon(spec)] as const;
}

/** `flows.node_state` — one entry per (ws, flow, tick). N cells on one flow share it; each slices its own
 *  node/port/path CLIENT-SIDE from the shared whole-flow read (scope goal 4). */
export function flowNodeStateKey(ws: string, flowId: string, tick: number) {
  return ["flows.node_state", ws, flowId, tick] as const;
}

/** `series.read` backfill — one entry per (ws, series). N cells on one series share one read (scope goal 4).
 *  The live SSE tail stays OUTSIDE the cache (state vs motion) — this keys only the history backfill. */
export function seriesReadKey(ws: string, series: string) {
  return ["series.read", ws, series] as const;
}

/** The source-picker bundle — one entry per ws, shared by the page-level and editor instances (goal 3). */
export function sourcePickerKey(ws: string) {
  return ["source-picker", ws] as const;
}

/** `datasource.list` — one entry per ws (the bundle and the Query-tab dropdown read the same key). */
export function datasourceListKey(ws: string) {
  return ["datasource.list", ws] as const;
}
