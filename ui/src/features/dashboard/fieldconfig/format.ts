// THE user-prefs formatting bridge (viz field-config scope, "THE user-prefs bridge"). This is the
// ONLY place in the viz layer that turns a canonical value + a `fieldConfig` unit/decimals into a
// display string. A renderer NEVER calls `toFixed` or bakes a unit string itself — it calls
// `formatValue` here. That keeps one formatter for the whole platform (the canonical-in / localized-out
// rule) and makes the `lb-prefs` swap data-only.
//
// SEQUENCING (the load-bearing decision — field-config scope, "Sequencing fallback"): `lb-prefs`
// (`format.quantity`/`format.number`/`format.datetime`) is NOT shipped yet. Until it lands this file
// renders the FALLBACK — the canonical value + a static unit label + a local `decimals` round. The
// call SITE is already `format.*`-shaped (a `from_unit`/`dimension`/`decimals` request), so when the
// prefs tools appear we replace `fallbackFormat` with the real MCP call here — no `fieldConfig` schema
// change and no re-save. The fallback is honest (it says what it is via `formatted=false`-style intent),
// it never invents a converted value, and it lives behind the same function the renderer already calls.

import type { FieldOptions } from "@/lib/dashboard";
import { resolveUnit, type UnitMapping } from "./units";

/** A formatted value for display — the localized string + the parts a renderer/legend may want
 *  separately (the numeric text and the unit suffix). `viaPrefs` records whether the real prefs
 *  formatter produced it (false today — the fallback); it lets a test assert the swap point. */
export interface FormattedValue {
  /** The display string (number + unit), ready to render. */
  text: string;
  /** The numeric portion alone (localized number, no unit) — for axis ticks / compact displays. */
  number: string;
  /** The unit suffix alone (may be empty). */
  unit: string;
  /** Whether the user-prefs `format.*` tool produced this (false = fallback until `lb-prefs` ships). */
  viaPrefs: boolean;
}

/** Round to `decimals` if given, else a sane default (2 for non-integers, 0 for integers). This is
 *  the FALLBACK number renderer — it will be replaced by `format.number({decimals})` in the viewer's
 *  locale once prefs ships. It does NOT convert units (the fallback shows canonical + static label). */
function fallbackNumber(value: number, decimals?: number): string {
  if (!Number.isFinite(value)) return String(value);
  if (decimals === undefined) {
    return Number.isInteger(value) ? String(value) : String(Math.round(value * 100) / 100);
  }
  return value.toFixed(decimals);
}

/** Render one canonical numeric value through a field's options. Today: the fallback (canonical value
 *  + static unit label + local round). Tomorrow: the real `format.quantity`/`format.number`/
 *  `format.datetime` call in the viewer's prefs — swapped in HERE, behind this same signature. */
export function formatValue(value: unknown, opts: FieldOptions | undefined): FormattedValue {
  const decimals = opts?.decimals;
  const mapping = resolveUnit(opts?.unit);

  // Non-numeric value: no number math — show as text, with the unit's noValue handled by the caller.
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return { text: value == null ? "" : String(value), number: value == null ? "" : String(value), unit: "", viaPrefs: false };
  }

  return fallbackFormat(value, mapping, decimals);
}

/** The sequencing fallback — canonical value + static unit, no conversion (field-config scope). When
 *  `lb-prefs` ships, this function becomes the real `format.*` dispatch; the call site is unchanged. */
function fallbackFormat(value: number, mapping: UnitMapping, decimals?: number): FormattedValue {
  const number = fallbackNumber(value, decimals);
  switch (mapping.kind) {
    case "quantity":
      // FALLBACK: show the canonical number + the source-unit label (NO conversion — prefs does that).
      // The label is the unit's display string, never a hard-coded per-renderer string.
      return { text: `${number} ${mapping.label}`.trim(), number, unit: mapping.label, viaPrefs: false };
    case "number":
      return { text: `${number}${mapping.suffix ?? ""}`, number, unit: (mapping.suffix ?? "").trim(), viaPrefs: false };
    case "datetime": {
      // FALLBACK: a plain ISO-ish render (the prefs formatter localizes tz + style — see
      // `useFormattedValue`). Honor the declared `dateUnit` so a flow epoch-SECONDS value shows the
      // right year here too (not 1970) while the async prefs render settles.
      const ms = mapping.dateUnit === "s" ? value * 1000 : value;
      return { text: new Date(ms).toISOString(), number, unit: "", viaPrefs: false };
    }
    case "none":
    default:
      return { text: number, number, unit: "", viaPrefs: false };
  }
}
