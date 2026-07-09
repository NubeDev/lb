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
 *  string sink. `raw` is the unquoted default. The advanced-variables scope widens this toward Grafana's
 *  set: `regex`/`glob` (regex-/glob-safe alternation over a multi-value), `percentencode` (URL-encode),
 *  `sqlstring` (single-quote-escaped SQL literal), `distributed` (Graphite `name,var=a,var=b`). */
export type FormatHint =
  | "json"
  | "csv"
  | "singlequote"
  | "doublequote"
  | "pipe"
  | "raw"
  | "regex"
  | "glob"
  | "percentencode"
  | "sqlstring"
  | "distributed";

/** A variable's resolver kind. ALL map to ONE `{tool,args}` resolver at runtime (query/source/datasource)
 *  or a static form (custom/text/const/interval) — no per-type code path (scope: "one resolver"). The
 *  `datasource` type still resolves through the one `{tool,args}` path (a fixed `datasource.list` tool). */
export type VariableType =
  | "query"
  | "custom"
  | "text"
  | "const"
  | "interval"
  | "source"
  | "datasource";

/** A resolved/static option — `text` (what the bar shows) may differ from `value` (what interpolates).
 *  A bare `custom` string is `{text:v, value:v}`; a `label : value` custom string splits the two; a query
 *  variable's `(?<text>)`/`(?<value>)` regex capture groups produce the split. */
export interface VariableOption {
  text: string;
  value: string;
  selected?: boolean;
}

/** How a query variable's `regex` is applied to each resolved row (advanced-variables scope). `value`
 *  (default) filters/captures against the row's value; `text` against its display text. */
export type RegexApplyTo = "value" | "text";

/** The option sort order (advanced-variables scope). `none` keeps insertion order (Grafana disabled). */
export type VariableSort =
  | "none"
  | "alphaAsc"
  | "alphaDesc"
  | "numAsc"
  | "numDesc"
  | "alphaCiAsc"
  | "alphaCiDesc";

/** When a variable re-resolves its options (advanced-variables scope). Distinct from the dashboard-level
 *  auto-refresh which re-runs *panels*. `never` resolves once (static-ish); `onLoad` on every dashboard
 *  load; `onTimeRange` additionally when the time range changes. */
export type VariableRefresh = "never" | "onLoad" | "onTimeRange";

/** The bar visibility of a variable (advanced-variables scope). `dontHide` shows label + control;
 *  `hideLabel` shows only the control; `hideVariable` hides it entirely (still resolves + interpolates). */
export type VariableHide = "dontHide" | "hideLabel" | "hideVariable";

/** A dashboard variable DEFINITION (lives on the dashboard record; the SELECTION lives in the URL). */
export interface Variable {
  /** The reference name — `$name` / `${name}` / `[[name]]`. */
  name: string;
  /** A human label for the bar dropdown (defaults to `name`). */
  label?: string;
  /** An optional icon (a stable icon-lib name, e.g. `"map-pin"`) shown on the bar before the label
   *  (advanced-variables scope). Additive/optional — a pre-icon record loads unchanged. */
  icon?: string;
  type: VariableType;
  /** `query`/`source`/`datasource`: the resolver — a granted MCP tool whose rows become the option list.
   *  A `datasource` variable resolves against a fixed `datasource.list` tool (build-time default). */
  query?: { tool: string; args?: Record<string, unknown> };
  /** `custom`: a static option list. A bare string is `{text:v,value:v}`; a `label : value` string splits
   *  display text from interpolated value (advanced-variables scope). Kept readable for round-trip. */
  custom?: string[];
  /** The resolved/static option list (`{text,value}`), when text ≠ value (advanced-variables scope).
   *  Additive: a pre-advanced record has none and falls back to `custom`/`interval` bare strings. */
  options?: VariableOption[];
  /** `text`: a free-textbox default. */
  text?: string;
  /** `const`: a hidden fixed value. */
  const?: string;
  /** `interval`: a duration list (feeds `$__interval`). */
  interval?: string[];
  /** Selection affordances. */
  multi?: boolean;
  includeAll?: boolean;
  /** A literal emitted when "All" is selected (advanced-variables scope) — e.g. `.*` for a regex sink —
   *  instead of expanding every option. Consumed by the interpolator's All-expansion. */
  allValue?: string;
  /** A regex applied to each resolved query row (advanced-variables scope): filters rows that don't
   *  match, and — with `(?<text>)`/`(?<value>)` named capture groups — splits display text from value. */
  regex?: string;
  /** Which side of a resolved row the `regex` applies to (advanced-variables scope; default `value`). */
  regexApplyTo?: RegexApplyTo;
  /** The option sort order (advanced-variables scope; default `none` = insertion order). */
  sort?: VariableSort;
  /** When the options re-resolve (advanced-variables scope; default `onLoad`). */
  refresh?: VariableRefresh;
  /** Bar visibility (advanced-variables scope; default `dontHide`). */
  hide?: VariableHide;
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
