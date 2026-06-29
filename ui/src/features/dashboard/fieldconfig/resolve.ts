// Resolve the EFFECTIVE field options for one field = `fieldConfig.defaults` merged with every
// override whose matcher hits the field (viz field-config scope: defaults + per-field overrides). The
// dotted Grafana property ids (`unit`, `decimals`, `custom.lineWidth`, …) are applied verbatim
// (Resolved decision: "Accept Grafana's dotted override ids verbatim"). Later overrides win over
// earlier ones over the defaults (Grafana's last-wins order).
//
// One responsibility: produce one field's FieldOptions. Matcher evaluation is `matchers.ts`'s job.

import type { FieldConfig, FieldOptions } from "@/lib/dashboard";
import { matchesField, type FieldDesc } from "./matchers";

/** Set a dotted property id onto a `FieldOptions` (mutating a working copy). `custom.lineWidth`
 *  writes into the `custom` bag; a bare `unit`/`decimals`/`thresholds`/… writes the top-level field. */
function setProperty(opts: FieldOptions, id: string, value: unknown): void {
  if (id.startsWith("custom.")) {
    opts.custom = { ...(opts.custom ?? {}), [id.slice("custom.".length)]: value };
    return;
  }
  // Top-level field-option id (unit/decimals/min/max/thresholds/mappings/color/displayName/noValue).
  (opts as Record<string, unknown>)[id] = value;
}

/** The effective options for `field` = defaults + every matching override's properties (in order). */
export function resolveFieldOptions(fc: FieldConfig | undefined, field: FieldDesc): FieldOptions {
  const out: FieldOptions = { ...(fc?.defaults ?? {}) };
  if (out.custom) out.custom = { ...out.custom };
  for (const over of fc?.overrides ?? []) {
    if (!matchesField(field, over.matcher)) continue;
    for (const prop of over.properties) setProperty(out, prop.id, prop.value);
  }
  return out;
}
