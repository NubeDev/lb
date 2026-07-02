// `resolveBindings` — pure JSON-Pointer (RFC 6901) resolution of a component's props against the
// surface data model. A prop that is a `{$bind}` is swapped for the pointed-at value; unresolvable
// pointers resolve to `undefined` (the catalog renders "no value" — never a throw at view time). Deep:
// bindings nested inside array/object props are resolved too. This is the ONLY data lookup the render
// path does — no adapter, no normalize (genui-scope "The render path carries no adapter").

import type { DataModel, PropValue } from "./types";
import { isBinding } from "./types";

/** Resolve a single JSON Pointer against `data`. `""` = the whole document. Returns `undefined` for any
 *  missing segment (orphan-tolerant). Handles `~1`→`/` and `~0`→`~` unescaping and numeric array
 *  indices. */
export function resolvePointer(data: unknown, pointer: string): unknown {
  if (pointer === "") return data;
  if (!pointer.startsWith("/")) return undefined;
  const parts = pointer
    .slice(1)
    .split("/")
    .map((p) => p.replace(/~1/g, "/").replace(/~0/g, "~"));
  let cur: unknown = data;
  for (const part of parts) {
    if (cur === null || cur === undefined) return undefined;
    if (Array.isArray(cur)) {
      const idx = Number(part);
      if (!Number.isInteger(idx) || idx < 0 || idx >= cur.length) return undefined;
      cur = cur[idx];
    } else if (typeof cur === "object") {
      cur = (cur as Record<string, unknown>)[part];
    } else {
      return undefined;
    }
  }
  return cur;
}

/** Deep-resolve every `{$bind}` in a prop value against the data model. Literals pass through. */
export function resolveValue(value: PropValue, data: DataModel): unknown {
  if (isBinding(value)) return resolvePointer(data, value.$bind);
  if (Array.isArray(value)) return value.map((v) => resolveValue(v, data));
  if (value !== null && typeof value === "object") {
    const out: Record<string, unknown> = {};
    for (const [k, v] of Object.entries(value)) out[k] = resolveValue(v as PropValue, data);
    return out;
  }
  return value;
}

/** Resolve a whole props bag. */
export function resolveBindings(
  props: Record<string, PropValue> | undefined,
  data: DataModel,
): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(props ?? {})) out[k] = resolveValue(v, data);
  return out;
}
