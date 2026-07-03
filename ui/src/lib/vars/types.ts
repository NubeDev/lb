// The shared variable types (widget-config-vars scope, "The shared `vars` library"). Pure TS — NO
// React, NO `@/` shell imports — so this module bundles into BOTH the shell and a federated extension
// remote (and is exposed as a federation-shared singleton, like React). The moment an extension links
// it, this is a FROZEN contract; the `VARS_LIB_V` version below is the freeze marker — a future shape
// change bumps it and a receiver rejects an unknown major.
//
// One model: a Variable is a NAME bound to a RESOLVER (`{tool,args}` or a static form). The resolved
// SELECTION (a value or a multi-value list) plus the shell-resolved BUILTINS form a `VarScope`, which
// `interpolate`/`interpolateArgs` substitute everywhere (cell args, control actions, SQL vars, JSON
// payloads). Definitions live on the dashboard record; the selection lives in the URL (Slice 2).

/** The frozen contract version. Bump the major on any breaking shape change to `VarScope`/the resolved
 *  value shape; a receiver (an extension linking this lib) rejects an unknown major. */
export const VARS_LIB_V = 1;

/** A Grafana-style format hint (`${var:json}` etc.) — how a (possibly multi) value renders into a
 *  string sink. `raw` is the unquoted default; the richer `{a,b}`/regex/glob forms are named follow-ups. */
export type FormatHint = "json" | "csv" | "singlequote" | "doublequote" | "pipe" | "raw";

/** A variable's resolver kind. ALL map to ONE `{tool,args}` resolver at runtime (query/source) or a
 *  static form (custom/text/const/interval) — no per-type code path (scope: "one resolver"). */
export type VariableType = "query" | "custom" | "text" | "const" | "interval" | "source";

/** A dashboard variable DEFINITION (lives on the dashboard record; the SELECTION lives in the URL). */
export interface Variable {
  /** The reference name — `$name` / `${name}` / `[[name]]`. */
  name: string;
  /** A human label for the bar dropdown (defaults to `name`). */
  label?: string;
  type: VariableType;
  /** `query`/`source`: the resolver — a granted MCP tool whose rows become the option list. */
  query?: { tool: string; args?: Record<string, unknown> };
  /** `custom`: a static option list. */
  custom?: string[];
  /** `text`: a free-textbox default. */
  text?: string;
  /** `const`: a hidden fixed value. */
  const?: string;
  /** `interval`: a duration list (feeds `$__interval`). */
  interval?: string[];
  /** Selection affordances. */
  multi?: boolean;
  includeAll?: boolean;
  /** reusable-pages scope: marks this variable a **page parameter**. A `required` variable left unbound
   *  (no `?var-` URL value, no default) makes the dashboard render the honest "select a `<label>`" gate
   *  (`RequiredVarGate`) instead of firing cells with a `$name`-literal — this is what turns an ordinary
   *  dashboard into a *template*. Additive/optional — a pre-reusable-pages record loads unchanged. */
  required?: boolean;
}

/** The shell-resolved built-in globals (`$__from`/`${__user.login}`/`${__workspace}`/…). PURE given
 *  trusted inputs — the shell supplies them from the verified token + the URL time range, NEVER a cell
 *  or an iframe (un-spoofable). A flat string map keyed by the built-in's bare name (no leading `$__`). */
export type Builtins = Record<string, string>;

/** A resolved variable VALUE — a single value, or a multi-value list (multi/include-all selections). */
export type VarValue = string | string[];

/** The fully-resolved scope `interpolate`/`interpolateArgs` substitute against: the user-variable
 *  selections (by name) + the built-ins. This is the contract handed to a widget as `ctx.vars` (Slice 3). */
export interface VarScope {
  /** The resolved user-variable selections, keyed by variable name. */
  values: Record<string, VarValue>;
  /** The shell-resolved built-ins (token + time range derived). */
  builtins: Builtins;
}

/** An empty scope — handy for tests and the no-variables dashboard. */
export function emptyScope(): VarScope {
  return { values: {}, builtins: {} };
}
