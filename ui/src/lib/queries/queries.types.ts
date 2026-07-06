// The query wire shapes — mirror the host's `query.*` MCP verbs + the `query:{ws}:{id}` record
// (query scope / `rust/crates/host/src/query/`). A saved query is a persisted PRQL/raw body +
// declared params + a target (`"platform"` or `"datasource:<name>"`); a run returns the SAME
// `{columns, rows}` shape `store.query`/`federation.query` yield. The shell reaches the verbs
// through the host-mediated MCP bridge (`mcp_call`), so this client adds NO gateway route — rule 7
// (MCP is the universal contract); the workspace + caps come from the token (§7), re-checked per
// call. `query.run` COMPOSES the target's cap, it never widens it (rule 5): the caller needs
// `mcp:query.run:call` AND the underlying target cap.

/** The authoring language of a saved query. `prql` compiles to the target's dialect; `raw` carries
 *  target-native text verbatim (raw SurrealQL for platform, raw SQL for a datasource). */
export type QueryLang = "prql" | "raw";

/** The target a saved query runs against. `"platform"` → `store.query` (SurrealDB-native);
 *  `"datasource:<name>"` → `federation.query` over a registered external source. */
export type QueryTarget = string;

/** The persisted shape of a saved query (`query:{ws}:{id}`). Mirrors the host `SavedQuery` record
 *  1:1. `removed` is the soft-delete tombstone (absent from `query.list`). */
export interface SavedQuery {
  id: string;
  name: string;
  description: string;
  lang: QueryLang;
  text: string;
  target: QueryTarget;
  params: string[];
  tag: string;
  removed?: boolean;
  ts: number;
}

/** A roster row from `query.list` — the minimum a list needs (no text, no result data). Mirrors the
 *  host `QuerySummary`. This is the structural superset of the source-picker's `QuerySummary`
 *  (`{id, name, target?}`); the shell adapter projects onto the package's shape. */
export interface QuerySummary {
  id: string;
  name: string;
  target: QueryTarget;
  lang: QueryLang;
  ts: number;
}

/** The `{columns, rows}` shape both `store.query` and `federation.query` (and thus `query.run`)
 *  return. A row is a column→value map (platform) or a column-aligned array (some federation
 *  engines re-project Arrow objects to arrays). */
export interface QueryRunResult {
  columns: string[];
  rows: (Record<string, unknown> | unknown[])[];
}

/** The `query.compile` dry-run result: the compiled SQL for the target's dialect (or the verbatim
 *  raw text). Authoring feedback only — no rows. */
export interface QueryCompileResult {
  sql: string;
}
