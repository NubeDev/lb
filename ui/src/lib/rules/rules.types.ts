// The rules wire shapes — mirror the gateway's `rules.*` routes + the host `SavedRule` record and the
// `rules.run` result (rules-workbench scope, Phase 1). A saved rule is a persisted Rhai body + declared
// params; a run returns a typed `RuleOutput` (rendered three ways by `kind`) plus findings, log, and a
// budget readout.

/** The declared type of a param — steers the authoring input + the value coercion (mirrors the node's
 *  `ParamKind`). Absent on a legacy `{name,label}` record → treated as `"text"`. */
export type ParamKind = "text" | "number" | "date" | "enum";

/** A declared parameter of a saved rule — name + optional human label + its type (mirrors the node's
 *  `RuleParam`). `kind`/`required`/`options` are optional so a legacy record round-trips unchanged. */
export interface RuleParam {
  name: string;
  label?: string;
  kind?: ParamKind;
  required?: boolean;
  /** Allowed values for an `enum` param (ignored otherwise). */
  options?: string[];
}

/** The persisted shape of a saved rule (`rule:{ws}:{id}`). `deleted` is the soft-delete tombstone. */
export interface SavedRule {
  id: string;
  name: string;
  body: string;
  params: RuleParam[];
  deleted?: boolean;
}

/** The typed result of a run, discriminated on `kind` — rendered one way per kind (FILE-LAYOUT: one
 *  render component per kind). `scalar` is a single value; `grid` is columns + rows; `findings` means
 *  the result is the emitted findings list; `nothing` is an empty run. */
export type RuleOutput =
  | { kind: "scalar"; value: unknown }
  // A row is EITHER an object keyed by column name (platform/SurrealDB) OR a column-aligned array
  // (federation/datasource — the sidecar re-projects Arrow objects to arrays). `GridTable.cellAt`
  // reads both; declaring only the object shape is why federated grids rendered every cell NULL.
  | { kind: "grid"; columns: string[]; rows: (Record<string, unknown> | unknown[])[] }
  | { kind: "findings" }
  | { kind: "nothing" };

/** One emitted finding (`emit`/`alert`). `level` (`info|warning|critical`) is lifted for colouring;
 *  the whole emitted map rides through as `data`, with `data.alert === true` marking an alert. */
export interface Finding {
  level: string;
  data: Record<string, unknown> & { alert?: boolean };
}

/** One `log(...)` line collected during a run. */
export interface LogLine {
  level: string;
  message: string;
}

/** The per-run AI spend, surfaced for observability (calls + tokens). */
export interface AiBudget {
  calls: number;
  tokens: number;
}

/** The full `rules.run` result: the typed output + findings + log + the budget readout (`ms` + `ai`). */
export interface RunResult {
  output: RuleOutput;
  findings: Finding[];
  log: LogLine[];
  ms: number;
  ai: AiBudget;
}
