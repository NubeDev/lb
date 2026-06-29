// The Grafana `fieldConfig` shapes — adopted verbatim (viz field-config scope, "The shapes (owned
// here)"). A field's option set: unit/decimals/min-max/thresholds/value-mappings/color/displayName,
// plus per-field `overrides[]` with matchers. These are PURE DATA — how a value RENDERS through them
// (the user-prefs bridge) lives in `features/dashboard/fieldconfig/*`, never in a type file.
//
// We own the RECORD; we adopt Grafana's field NAMES so an imported panel's `fieldConfig` is a copy,
// not a re-model. Phase 1 ships `defaults` fully + `overrides[]` with `byName`/`byType` matchers;
// `byRegex`/`byFrameRefID` are accepted-but-deferred-render (named follow-ups), never silently wrong.

/** A matcher selecting which fields an override applies to. Phase 1 evaluates `byName`/`byType`. */
export interface Matcher {
  id: "byName" | "byType" | "byRegex" | "byFrameRefID";
  /** `byName`: the field name; `byType`: `"number"|"string"|"time"|…`; `byRegex`: the pattern. */
  options?: unknown;
}

/** The result of a value/range/regex/special mapping — text/color/icon to display instead of the raw value. */
export interface ValueMappingResult {
  text?: string;
  color?: string;
  icon?: string;
  index?: number;
}

/** A value mapping (Grafana's discriminated union). `value`/`range`/`special` render in Phase 1;
 *  `regex` is accepted but deferred (named follow-up) — it never silently mis-renders. */
export type ValueMapping =
  | { type: "value"; options: Record<string, ValueMappingResult> }
  | { type: "range"; options: { from: number | null; to: number | null; result: ValueMappingResult } }
  | { type: "regex"; options: { pattern: string; result: ValueMappingResult } }
  | {
      type: "special";
      options: { match: "true" | "false" | "null" | "nan" | "empty"; result: ValueMappingResult };
    };

/** Step coloring. The first step's `value` is always `null` (-∞), per Grafana. */
export interface ThresholdsConfig {
  mode: "absolute" | "percentage";
  steps: Array<{ value: number | null; color: string }>;
}

/** Grafana field-color mode ids (Phase 1 renders `thresholds`/`fixed`/`palette-classic`; others map
 *  to the accent token until their phase). */
export type FieldColorModeId =
  | "thresholds"
  | "fixed"
  | "palette-classic"
  | "palette-classic-by-name"
  | "continuous-GrYlRd"
  | "continuous-RdYlGr"
  | "continuous-viridis"
  | "shades";

export interface FieldColor {
  mode: FieldColorModeId;
  fixedColor?: string;
  seriesBy?: "last" | "min" | "max";
}

/** The per-field option set — Grafana's `FieldConfig` defaults. The `custom` bag holds per-view draw
 *  fields (lineWidth/fillOpacity/drawStyle/axis…), owned by the chart-types layer. */
export interface FieldOptions {
  displayName?: string;
  description?: string;
  /** Grafana unit id (`celsius`/`bytes`/`percent`/`velocitykmh`/`time:…`). Mapped to a dimension by
   *  `fieldconfig/units.ts` and rendered through the user-prefs bridge (`fieldconfig/format.ts`). */
  unit?: string;
  decimals?: number;
  min?: number;
  max?: number;
  noValue?: string;
  mappings?: ValueMapping[];
  thresholds?: ThresholdsConfig;
  color?: FieldColor;
  /** Per-view field options (lineWidth, fillOpacity, drawStyle, axisPlacement…). Grafana's
   *  `fieldConfig.custom`; the chart-types layer owns the per-view schema. */
  custom?: Record<string, unknown>;
}

/** One per-field override: a matcher + the properties it sets (Grafana's `DynamicConfigValue[]`,
 *  with dotted ids like `custom.lineWidth` accepted verbatim so import is a copy). */
export interface FieldOverride {
  matcher: Matcher;
  properties: Array<{ id: string; value: unknown }>;
}

/** The whole field-config: shared defaults + per-field overrides. */
export interface FieldConfig {
  defaults: FieldOptions;
  overrides?: FieldOverride[];
}

/** A fresh, empty field-config (defaults only). */
export function emptyFieldConfig(): FieldConfig {
  return { defaults: {}, overrides: [] };
}
