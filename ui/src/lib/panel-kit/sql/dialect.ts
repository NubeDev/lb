// The dialect dispatch over the typed `SqlBuilderQuery` (query-builder-common scope). One
// responsibility: pick the right SQL emitter for the builder's datasource `kind`. The builder UI
// (VisualEditor / SqlQueryEditor) is generic; only the dialect emitter differs. SurrealDB's
// `store.query` speaks SurrealQL; an external federation source (`federation.query` — sqlite /
// postgres / timescale) speaks standard SQL. The `kind` is config data, never a hardcoded
// datasource name (rule 10) — `emitSql` is keyed on the dialect, not on "demo-buildings" or any
// other id.
//
// v1 ships TWO emitters (one file each, FILE-LAYOUT): `toSurrealQL` (SurrealDB — `math::sum`,
// bare identifiers, `count()`) and `toStandardSql` (ANSI SELECT — `SUM("col")`, double-quoted
// identifiers, `COUNT(*)`). All three federation kinds speak near-ANSI for the SELECT subset the
// builder can express; split into per-kind files only when a real delta forces it (scope OQ #1 —
// e.g. timescale `time_bucket()` for the chart time-series format hint).

import type { SqlBuilderQuery } from "./query";
import { toSurrealQL } from "./toSurrealQL";
import { toStandardSql } from "./toStandardSql";

/** Which SQL dialect a builder target emits. `surreal` for native `store.query`;
 *  `standard` for a registered federation source (sqlite / postgres / timescale). */
export type SqlDialect = "surreal" | "standard";

/** Render the typed builder query to a SQL string for `dialect`. Returns `""` if no table is chosen
 *  yet (the builder is incomplete — the caller shows nothing to run). The returned string is still
 *  parse-allowlisted + bounded + workspace-walled by the host (`store.query` / `federation.query`)
 *  — the boundary, not this emitter. */
export function emitSql(dialect: SqlDialect, query: SqlBuilderQuery): string {
  return dialect === "surreal" ? toSurrealQL(query) : toStandardSql(query);
}
