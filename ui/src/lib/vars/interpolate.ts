// The string interpolation core (widget-config-vars scope, "interpolate.ts") — the FROZEN heart of the
// shared vars library. `interpolate(template, scope)` substitutes the three Grafana reference forms
// (`$var`, `${var}`, `[[var]]`), the format hints (`${var:json|csv|singlequote|doublequote|pipe|raw}`),
// and multi-value selections, leaving an UNKNOWN variable LITERAL (Grafana behavior — a shared link
// always renders). Pure TS, no React.
//
// Multi-value rendering (csv/pipe/json shipped; the `{a,b}`/regex/glob forms are NAMED follow-ups, NOT
// built here). A single value with no format hint renders as itself; a multi-value with no hint joins
// with commas (Grafana's default for `${var}` in most sinks).

import type { FormatHint, VarScope, VarValue } from "./types";
import { isBuiltinName } from "./parse";

const NAME = "[A-Za-z_][\\w.]*";
const FORMAT = "(?::([a-z]+))?";
// Matches, in priority order: ${name:fmt}  |  [[name:fmt]]  |  $name  (bare, no brace).
const RE = new RegExp(
  `\\$\\{(${NAME})${FORMAT}\\}` +
    `|\\[\\[(${NAME})${FORMAT}\\]\\]` +
    `|\\$(${NAME})`,
  "g",
);

/** Look a name up in the scope: a user variable (`values`) first, then a built-in (`builtins`). Returns
 *  `undefined` if the name is neither — the caller leaves the reference literal. */
function lookup(name: string, scope: VarScope): VarValue | undefined {
  if (name in scope.values) return scope.values[name];
  // Built-ins are keyed by their bare name (no `__` stripped — `__from`, `__user.login`).
  if (isBuiltinName(name) && name in scope.builtins) return scope.builtins[name];
  return undefined;
}

/** Render a resolved value with a format hint. Multi-value aware (csv/pipe/json); a single value is
 *  treated as a one-element list for csv/pipe/json so the sink shape is consistent. */
export function formatValue(value: VarValue, hint: FormatHint | undefined): string {
  const list = Array.isArray(value) ? value : [value];
  switch (hint) {
    case "json":
      // A JSON-encoded scalar (single → quoted string) or array (multi) — for a JSON-text sink.
      return JSON.stringify(value);
    case "csv":
      return list.join(",");
    case "pipe":
      return list.join("|");
    case "singlequote":
      return list.map((v) => `'${String(v).replace(/'/g, "\\'")}'`).join(",");
    case "doublequote":
      return list.map((v) => `"${String(v).replace(/"/g, '\\"')}"`).join(",");
    case "raw":
      return list.join(",");
    default:
      // No hint: a single value is itself; a multi-value joins with commas (Grafana default).
      return Array.isArray(value) ? value.join(",") : value;
  }
}

/** Substitute every `$var`/`${var}`/`[[var]]` (with optional `:format`) in `template` using `scope`.
 *  An unknown variable is left exactly as written (Grafana behavior); never throws. */
export function interpolate(template: string, scope: VarScope): string {
  RE.lastIndex = 0;
  return template.replace(RE, (whole, n1, f1, n2, f2, n3) => {
    const name = n1 ?? n2 ?? n3;
    const hint = (f1 ?? f2) as FormatHint | undefined;
    const value = lookup(name, scope);
    if (value === undefined) return whole; // unknown → literal
    return formatValue(value, hint);
  });
}
