// Evaluate a field-override `Matcher` against a field (viz field-config scope, "The shapes"). Supports
// `byName` (exact field name), `byType` (field value kind), and `byRegex` (a JS regex over the field
// name — wired when the editor started authoring it in editor-parity step 4). `byFrameRefID` stays
// deferred (it needs the field's owning frame refId, which this per-field descriptor doesn't carry) —
// inert, not silently wrong. An invalid regex pattern never matches (never throws at render).
//
// One responsibility: matcher → does-this-field-hit. `resolveFieldOptions` (the merge of defaults +
// matched overrides) lives in `resolve.ts`.

import type { Matcher } from "@/lib/dashboard";

/** A minimal field descriptor an override matches against — its name + inferred type. */
export interface FieldDesc {
  name: string;
  /** `"number" | "string" | "time" | "boolean"` — inferred from the field's values. */
  type: string;
}

/** Does `field` satisfy `matcher`? `byName`/`byType`/`byRegex`. `byFrameRefID` is deferred (needs frame
 *  context). An empty/invalid pattern never matches. */
export function matchesField(field: FieldDesc, matcher: Matcher): boolean {
  switch (matcher.id) {
    case "byName":
      return field.name === String(matcher.options ?? "");
    case "byType":
      return field.type === String(matcher.options ?? "");
    case "byRegexp":
      return matchesRegex(field.name, matcher.options);
    case "byFrameRefID":
      // Deferred (needs the field's owning frame refId — not carried by FieldDesc). Inert, not wrong.
      return false;
    default:
      return false;
  }
}

/** Test a field name against a `byRegex` pattern. Accepts a bare pattern or a `/pattern/flags` literal.
 *  An empty or invalid pattern returns false (never throws at render). */
function matchesRegex(name: string, options: unknown): boolean {
  const raw = String(options ?? "").trim();
  if (!raw) return false;
  try {
    const slashed = raw.match(/^\/(.*)\/([a-z]*)$/i);
    const re = slashed ? new RegExp(slashed[1], slashed[2]) : new RegExp(raw);
    return re.test(name);
  } catch {
    return false;
  }
}

/** Infer a field's type from a sample value (for `byType`). */
export function inferType(value: unknown): string {
  if (typeof value === "number") return "number";
  if (typeof value === "boolean") return "boolean";
  return "string";
}
