// Apply a field's `ValueMapping[]` to a value → an optional display result (viz field-config scope,
// "The shapes"). A value/range/special mapping replaces the raw value with `{text,color,icon}`; the
// FIRST matching mapping wins (Grafana order). `regex` is accepted but DEFERRED in Phase 1 (named
// follow-up) — it never silently mis-renders; it simply doesn't match here.
//
// One responsibility: value → mapping result. Color resolution (name → token) is `color.ts`'s job;
// the caller applies the returned color through `resolveColor`.

import type { ValueMapping, ValueMappingResult } from "@/lib/dashboard";

/** The first mapping that matches `value`, or `null`. Handles `value` (exact, by stringified key),
 *  `range` (numeric, inclusive nulls = ±∞), and `special` (null/nan/empty/true/false). `regex` is
 *  deferred (returns no match) — honest, never wrong. */
export function applyMappings(value: unknown, mappings: ValueMapping[] | undefined): ValueMappingResult | null {
  if (!mappings || mappings.length === 0) return null;
  for (const m of mappings) {
    const hit = matchOne(value, m);
    if (hit) return hit;
  }
  return null;
}

function matchOne(value: unknown, m: ValueMapping): ValueMappingResult | null {
  switch (m.type) {
    case "value": {
      const key = String(value);
      return m.options[key] ?? null;
    }
    case "range": {
      if (typeof value !== "number" || !Number.isFinite(value)) return null;
      const { from, to, result } = m.options;
      if ((from == null || value >= from) && (to == null || value <= to)) return result;
      return null;
    }
    case "special": {
      const { match, result } = m.options;
      if (match === "null" && value == null) return result;
      if (match === "nan" && typeof value === "number" && Number.isNaN(value)) return result;
      if (match === "empty" && value === "") return result;
      if (match === "true" && value === true) return result;
      if (match === "false" && value === false) return result;
      return null;
    }
    case "regex":
      // Deferred (Phase 2 with byRegex). Accepted on import, not rendered here — never mis-matches.
      return null;
    default:
      return null;
  }
}
