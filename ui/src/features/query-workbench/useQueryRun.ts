// The query-run dispatch hook (query-workbench-view scope, slice 3). One responsibility: run the
// authored SQL through the RIGHT gated engine for the workbench's `source` — `store.query` for the
// platform's native SurrealDB store (`source === "surreal-local"`), `federation.query` for a
// registered external datasource (any other `source`). Both verbs ride the real `POST /mcp/call`
// bridge, capability-gated server-side + workspace-pinned from the token (the hard wall, §7) +
// SELECT-only validated at the host (parse-allowlist for surreal, sidecar validation for
// federation). No second verb, no batch, no write path — a run is one bounded SELECT (rule 2).
//
// The two engine results are normalized to the SAME `{columns, rows}` shape the shipped
// `QueryResults` grid renders, so the workbench's results area is engine-agnostic. A deny / failure
// is surfaced as an honest error string (never a throw into the render path, never fabricated rows)
// — the `mcp:store.query:call` / `mcp:federation.query:call` cap is the gate.

import { useCallback, useState } from "react";

import { runQuery } from "@/lib/dashboard/sql.api";
import { runFederationQuery } from "@/lib/datasources";
import type { FederationQueryResult } from "@/lib/datasources";

/** The sentinel `source` value that selects the platform's native SurrealDB store (`store.query`).
 *  Any other `source` string is a registered federation datasource name. Config data, never an
 *  extension id (rule 10) — the dispatch is keyed on this value + the datasource `kind`, never on a
 *  datasource name. */
export const SURREAL_LOCAL = "surreal-local";

/** Pure dispatch decision — extracted so a unit test can pin it without faking the transport
 *  (rule 9: no mocks for the run path; the real-gateway test exercises the verbs end to end).
 *  `"surreal-local"` ⇒ the surreal dialect + `store.query`; any other source ⇒ the standard
 *  dialect + `federation.query`. */
export function runKindFor(source: string): "surreal" | "federation" {
  return source === SURREAL_LOCAL ? "surreal" : "federation";
}

export interface QueryRunState {
  /** The last run's result (`{columns, rows}`), or null before the first run / after a deny. */
  result: FederationQueryResult | null;
  loading: boolean;
  /** The verbatim host error on a deny/failure (never fabricated rows). Null on success. */
  error: string | null;
  /** The SQL that produced `result` (so the run bar can echo what was asked). */
  lastSql: string | null;
  /** Run `sql` through the right engine. Surfaces an error object on deny (no throw). */
  run: (sql: string) => Promise<void>;
  /** Drop the current result + error (e.g. on source switch). */
  reset: () => void;
}

/** The run hook. `source` is either {@link SURREAL_LOCAL} or a registered federation datasource
 *  name (workspace-pinned host-side). Dispatches to `store.query` vs `runFederationQuery` by the
 *  pure {@link runKindFor} decision. */
export function useQueryRun(source: string): QueryRunState {
  const [result, setResult] = useState<FederationQueryResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [lastSql, setLastSql] = useState<string | null>(null);

  const run = useCallback(
    async (sql: string) => {
      const trimmed = sql.trim();
      if (!trimmed) return;
      setLoading(true);
      setError(null);
      try {
        // Both engines return `{columns, rows}` (the surreal `QueryResult` is structurally identical
        // to `FederationQueryResult` — a `{columns, rows}` frame; the cast is shape-only, not a lie).
        const r =
          runKindFor(source) === "surreal"
            ? (await runQuery(trimmed) as FederationQueryResult)
            : await runFederationQuery(source, trimmed);
        setResult(r);
        setLastSql(trimmed);
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
        setResult(null);
      } finally {
        setLoading(false);
      }
    },
    [source],
  );

  const reset = useCallback(() => {
    setResult(null);
    setError(null);
    setLastSql(null);
  }, []);

  return { result, loading, error, lastSql, run, reset };
}
