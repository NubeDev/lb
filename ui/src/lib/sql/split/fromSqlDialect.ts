// Ported verbatim from Tabularis (github.com/TabularisDB/tabularis), Apache-2.0.
// Pure TS, zero coupling — copied file-by-file per the query-builder 10x copy discipline.
import type { Dialect } from "./index";
import type { SqlDialect } from "@/lib/panel-kit/sql/dialect";

/** Map our SqlDialect to the splitter's Dialect. standard → postgres (the safe superset);
 *  surreal → generic (SurrealQL is ;-separated for our subset). */
export function toSplitterDialect(d: SqlDialect): Dialect {
  return d === "surreal" ? "generic" : "postgres";
}
