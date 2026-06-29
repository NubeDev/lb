// Evaluate a field-override `Matcher` against a field (viz field-config scope, "The shapes"). Phase 1
// supports `byName` (exact field name) and `byType` (field value kind); `byRegex`/`byFrameRefID` are
// accepted-but-deferred (named follow-ups) — they never match here, so an override using them is inert
// rather than silently wrong.
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

/** Does `field` satisfy `matcher`? Phase 1: `byName`/`byType`. Deferred matchers return false. */
export function matchesField(field: FieldDesc, matcher: Matcher): boolean {
  switch (matcher.id) {
    case "byName":
      return field.name === String(matcher.options ?? "");
    case "byType":
      return field.type === String(matcher.options ?? "");
    case "byRegex":
    case "byFrameRefID":
      // Deferred (named follow-ups). Inert, not wrong.
      return false;
    default:
      return false;
  }
}

/** Infer a field's type from a sample value (for `byType`). */
export function inferType(value: unknown): string {
  if (typeof value === "number") return "number";
  if (typeof value === "boolean") return "boolean";
  return "string";
}
