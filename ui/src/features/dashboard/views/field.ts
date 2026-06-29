// The shared single-value field resolution for the single-stat-style panels (stat/gauge/bargauge/
// piechart). They all draw ONE field (the canonical numeric `value`), so they all resolve its effective
// `FieldOptions` (fieldConfig defaults + matching overrides) and its threshold/fixed color the same way.
// This is that one path — so a renderer never re-implements the fieldConfig math or, worse, bakes a
// color/unit string itself (the canonical-out rule; chart-types scope §"Per-field options come from
// fieldConfig via the EXISTING bridge").
//
// One responsibility: cell → the value field's resolved options + its color-for-a-value. Formatting is
// `format.ts`'s job; the value→color step is `thresholds.ts`/`color.ts`'s.

import type { Cell, FieldOptions } from "@/lib/dashboard";
import { cellFieldConfig } from "@/lib/dashboard";
import { resolveFieldOptions } from "../fieldconfig/resolve";
import { resolveFieldColor } from "../fieldconfig/color";
import { thresholdColor } from "../fieldconfig/thresholds";

/** The canonical field name a single-value panel reduces to (mirrors the timeseries `value` field). */
export const VALUE_FIELD = "value";

/** The effective `FieldOptions` for a cell's value field = fieldConfig.defaults + matching overrides. */
export function valueFieldOptions(cell: Cell): FieldOptions {
  return resolveFieldOptions(cellFieldConfig(cell), { name: VALUE_FIELD, type: "number" });
}

/** The classic categorical palette (theme tokens) for multi-slice/bar panels (piechart/barchart) when
 *  the field has no thresholds — Grafana's `palette-classic` analog over our resolved color tokens. */
const PALETTE = [
  "hsl(217 91% 60%)",
  "hsl(142 71% 45%)",
  "hsl(28 90% 55%)",
  "hsl(270 70% 60%)",
  "hsl(0 72% 51%)",
  "hsl(45 93% 47%)",
  "hsl(190 80% 50%)",
  "hsl(330 70% 55%)",
];

/** The color for one category (slice/bar) at `index` with `value`: the threshold step color when the
 *  field declares thresholds (so a categorical panel still honors thresholds), else the field's fixed
 *  color, else the classic palette rotated by index. Never a hard-coded per-renderer color. */
export function categoryColor(value: number, index: number, opts: FieldOptions): string {
  if (opts.thresholds && opts.thresholds.steps.length > 0) return valueColor(value, opts);
  if (opts.color?.mode === "fixed" && opts.color.fixedColor) return valueColor(value, opts);
  return PALETTE[index % PALETTE.length];
}

/** The color for a specific canonical `value` under a field's options: its threshold step color when
 *  thresholds are set (evaluated against canonical min/max bounds), else the field's fixed/accent color.
 *  The ONE place a single-stat panel turns a value into a color — never a hard-coded per-renderer color. */
export function valueColor(value: number, opts: FieldOptions): string {
  const base = resolveFieldColor(opts.color);
  if (opts.thresholds && opts.thresholds.steps.length > 0) {
    return thresholdColor(value, opts.thresholds, base, { min: opts.min, max: opts.max });
  }
  return base;
}
