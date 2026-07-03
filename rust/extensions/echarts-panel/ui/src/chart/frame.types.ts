// The LOCAL subset of the lb-viz Frame + Grafana FieldConfig shapes the chart tile consumes. This is a
// deliberate LOCAL COPY (the extension is a separate build; it cannot import from `ui/src/*`): it mirrors
//   • `ui/src/features/dashboard/builder/useVizQuery.ts` — the canonical column `Frame` (what `viz.query`
//     returns and the shell pushes in as `ctx.data`);
//   • `ui/src/lib/dashboard/fieldconfig.types.ts` — the per-field `FieldOptions` / `FieldConfig` the
//     Field tab authors and the shell hands in as `ctx.fieldConfig`.
// Keep this a SUBSET: only the fields the chart mapping actually reads. If the host contract widens, this
// copy is what to reconcile.

/** One column of a frame: a named, optionally-typed array of values. `type` is the lb-viz field type
 *  (`"time" | "number" | "string" | …`) — data, not identity. */
export interface Field {
  name: string;
  type?: string;
  values: unknown[];
}

/** A canonical column frame, exactly as `viz.query` returns it (the shape the shell pushes in). One
 *  frame carries N parallel fields; `length` (row count) is optional — derive from the longest field. */
export interface Frame {
  refId?: string;
  name?: string;
  fields: Field[];
  length?: number;
}

/** Step coloring (Grafana `ThresholdsConfig`). The first step's `value` is `null` (-∞). */
export interface ThresholdsConfig {
  mode: "absolute" | "percentage";
  steps: Array<{ value: number | null; color: string }>;
}

/** The per-field option set — the SUBSET of Grafana's `FieldConfig.defaults` this tile renders through:
 *  unit/decimals label the axis + tooltip, thresholds tint, `custom` carries per-view draw hints
 *  (drawStyle/legend) the mapping honours. Everything else on the real shape is ignored here (never
 *  silently mis-rendered). */
export interface FieldOptions {
  displayName?: string;
  unit?: string;
  decimals?: number;
  min?: number;
  max?: number;
  thresholds?: ThresholdsConfig;
  /** Per-view draw hints (Grafana `fieldConfig.custom`). This tile reads `drawStyle` ("line"|"bar")
   *  and `showLegend` when present; unknown keys are ignored. */
  custom?: Record<string, unknown>;
}

/** The whole field-config: shared defaults (this tile reads `defaults`; per-field `overrides[]` are a
 *  named follow-up — accepted-but-not-yet-applied, never silently wrong). */
export interface FieldConfig {
  defaults?: FieldOptions;
  overrides?: unknown[];
}
