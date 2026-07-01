// The ONE per-field presentation resolver (widget-kit scope, Phase 1) — the single place BOTH the request
// FORM (palette arg rail, reading `x-lb` hints) and the response TABLE (both table renderers, reading
// `fieldConfig` FieldOptions) turn a field's name + its author-declared hints into the RESOLVED
// {@link FieldPresentation} a surface renders. Funnelling both sides through here is what keeps a header
// and a form label from drifting (the scope's load-bearing "don't fork the formatter" discipline): a
// renderer that hand-labels a header outside this forks presentation again. One responsibility: resolve
// one field's presentation.

import type { FieldPresentation, FieldPresentationHints } from "@/lib/widgets/types";
import { humanize } from "./humanize";

/** Resolve `fieldName` + its `hints` into the presentation every surface renders. `label` is the author
 *  override (`label`, or its `fieldConfig` alias `displayName`) when set, else the {@link humanize}
 *  fallback (never empty). `hidden` reflects `hide` (default false). `order` is passed through as an
 *  OPTIONAL override (undefined → the caller keeps the field's natural order — never reordered implicitly).
 *
 *  `hidden` is PRESENTATION, NOT SECURITY (see {@link FieldPresentation}). */
export function resolveFieldPresentation(
  fieldName: string,
  hints: FieldPresentationHints | undefined,
): FieldPresentation {
  const label = hints?.label ?? hints?.displayName ?? humanize(fieldName);
  return {
    label,
    description: hints?.description,
    hidden: hints?.hide === true,
    order: hints?.order,
  };
}
