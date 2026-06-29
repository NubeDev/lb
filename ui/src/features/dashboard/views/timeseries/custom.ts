// The timeseries per-FIELD custom draw options (Grafana `fieldConfig.custom` for the timeseries
// panel — names verbatim from `common.GraphFieldConfig`). These are field options (they live in
// `fieldConfig.defaults.custom` / an override's `custom.*` properties), NOT per-viz options — Grafana
// puts draw-style here and we keep that line so import is 1:1 (chart-types scope, Risks: "options vs
// fieldConfig boundary drift"). One responsibility: the typed custom shape + its defaults + a reader.

export type GraphDrawStyle = "line" | "bars" | "points";
export type LineInterpolation = "linear" | "smooth" | "stepBefore" | "stepAfter";
export type GraphGradientMode = "none" | "opacity" | "hue" | "scheme";
export type AxisPlacement = "auto" | "left" | "right" | "hidden";

/** The timeseries field draw options (Grafana's `GraphFieldConfig`, Phase-1 subset). */
export interface TimeseriesFieldCustom {
  drawStyle: GraphDrawStyle;
  lineInterpolation: LineInterpolation;
  lineWidth: number;
  fillOpacity: number;
  gradientMode: GraphGradientMode;
  spanNulls: boolean;
  showPoints: "auto" | "always" | "never";
  axisPlacement: AxisPlacement;
}

/** Grafana's verbatim defaults for a timeseries field. */
export function defaultTimeseriesCustom(): TimeseriesFieldCustom {
  return {
    drawStyle: "line",
    lineInterpolation: "linear",
    lineWidth: 1,
    fillOpacity: 10,
    gradientMode: "none",
    spanNulls: false,
    showPoints: "auto",
    axisPlacement: "auto",
  };
}

/** Read a field's `custom` bag as `TimeseriesFieldCustom`, filling Grafana defaults for missing keys. */
export function readTimeseriesCustom(custom: Record<string, unknown> | undefined): TimeseriesFieldCustom {
  return { ...defaultTimeseriesCustom(), ...((custom ?? {}) as Partial<TimeseriesFieldCustom>) };
}
