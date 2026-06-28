// Deep, type-preserving substitution over a JSON value tree (widget-config-vars scope,
// "interpolateValue.ts"). `interpolateArgs(argsTree, scope)` is what a cell's `source.args`, a control's
// action args, and a JSON payload template run through before a bridge call. It GENERALIZES the shipped
// `views/argsTemplate.ts` `subst` (the control `{{value}}` slot): `{{value}}` is now the built-in
// `${__value}`, so the same library covers both. Pure TS, no React.
//
// Type-preservation (scope: "no string coercion"):
//   - A string leaf that is EXACTLY a single reference (`${var}` / `$var` / `[[var]]` / `{{value}}`,
//     no surrounding text) is replaced with the variable's RAW value — a multi-value becomes a real
//     ARRAY (so a JSON `IN`/array sink works), the special `__value`/`{{value}}` keeps its real type
//     (a number stays a number, a bool a bool).
//   - A string leaf with surrounding text (`"cpu.${host}"`) is string-interpolated (formats applied).
//   - Non-string leaves pass through untouched.

import type { VarScope } from "./types";
import { interpolate } from "./interpolate";
import { isBuiltinName } from "./parse";

/** A leaf that is exactly one reference, with no surrounding text. Captures the bare name. */
const SOLE_REF = /^(?:\$\{([A-Za-z_][\w.]*)\}|\[\[([A-Za-z_][\w.]*)\]\]|\$([A-Za-z_][\w.]*))$/;

/** Look up a name's raw value (user variable, else built-in), preserving its real type. */
function rawLookup(name: string, scope: VarScope): unknown {
  if (name in scope.values) return scope.values[name];
  if (isBuiltinName(name) && name in scope.builtins) return scope.builtins[name];
  return undefined;
}

/** Substitute one string leaf — raw (type-preserving) when it is a sole reference, else string-interp. */
function substLeaf(s: string, scope: VarScope, runtimeValue: unknown): unknown {
  // The control `{{value}}` slot (argsTemplate compat) and its built-in alias `${__value}` → the
  // runtime interaction value (a switch bool, a slider number), keeping its real type.
  if (s === "{{value}}" || s === "${__value}" || s === "$__value") {
    if (runtimeValue !== undefined) return runtimeValue;
    // No runtime value supplied: fall through to the built-in lookup (`__value` may be in builtins).
  }
  const m = SOLE_REF.exec(s);
  if (m) {
    const name = m[1] ?? m[2] ?? m[3];
    const raw = rawLookup(name, scope);
    if (raw !== undefined) return raw; // type-preserving (array stays array)
    return s; // unknown → literal
  }
  return interpolate(s, scope); // embedded reference(s) → string interpolation
}

/** Deep-substitute every string leaf in `argsTree` using `scope`. `runtimeValue`, when given, fills the
 *  control `{{value}}`/`${__value}` slot (the argsTemplate generalization); omit it for a plain args tree. */
export function interpolateArgs(argsTree: unknown, scope: VarScope, runtimeValue?: unknown): unknown {
  if (typeof argsTree === "string") return substLeaf(argsTree, scope, runtimeValue);
  if (Array.isArray(argsTree)) return argsTree.map((n) => interpolateArgs(n, scope, runtimeValue));
  if (argsTree && typeof argsTree === "object") {
    const out: Record<string, unknown> = {};
    for (const [k, v] of Object.entries(argsTree)) out[k] = interpolateArgs(v, scope, runtimeValue);
    return out;
  }
  return argsTree; // number / bool / null → untouched
}
