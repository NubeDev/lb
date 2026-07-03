// The timeseries GRAPH-STYLE + AXIS field options (editor-parity scope, steps 2/5) — Grafana's
// `custom.*` draw options for the timeseries panel (line width, fill opacity, gradient, interpolation,
// point size, span nulls; axis placement). They live in `fieldConfig.defaults.custom.*` (scope
// "fieldConfig", dotted `custom.` paths) so an override can set any of them per-field, exactly like
// Grafana. `views: ["timeseries"]` scopes them to the timeseries Field/per-viz tabs + override picker.
// One responsibility: the timeseries custom option catalog.

import type { OptionDef } from "../types";
import { defaultTimeseriesCustom } from "../../../views/timeseries/custom";

const D = defaultTimeseriesCustom();
const STYLES = "Graph styles";
const AXIS = "Axis";
const TS = ["timeseries" as const];

export const TIMESERIES_GRAPH_OPTIONS: OptionDef[] = [
  {
    id: "custom.drawStyle",
    label: "Style",
    group: STYLES,
    scope: "fieldConfig",
    views: TS,
    control: {
      kind: "select",
      choices: [
        { value: "line", label: "Lines" },
        { value: "bars", label: "Bars" },
        { value: "points", label: "Points" },
      ],
    },
    default: D.drawStyle,
    keywords: ["line", "bar", "point"],
  },
  {
    id: "custom.lineInterpolation",
    label: "Line interpolation",
    group: STYLES,
    scope: "fieldConfig",
    views: TS,
    control: {
      kind: "select",
      choices: [
        { value: "linear", label: "Linear" },
        { value: "smooth", label: "Smooth" },
        { value: "stepBefore", label: "Step before" },
        { value: "stepAfter", label: "Step after" },
      ],
    },
    default: D.lineInterpolation,
    keywords: ["smooth", "step", "curve"],
  },
  {
    id: "custom.lineWidth",
    label: "Line width",
    group: STYLES,
    scope: "fieldConfig",
    views: TS,
    control: { kind: "number", min: 0, max: 10, step: 1 },
    default: D.lineWidth,
  },
  {
    id: "custom.fillOpacity",
    label: "Fill opacity",
    group: STYLES,
    scope: "fieldConfig",
    views: TS,
    control: { kind: "number", min: 0, max: 100, step: 1 },
    default: D.fillOpacity,
    keywords: ["area", "transparency"],
  },
  {
    id: "custom.gradientMode",
    label: "Gradient mode",
    group: STYLES,
    scope: "fieldConfig",
    views: TS,
    control: {
      kind: "select",
      choices: [
        { value: "none", label: "None" },
        { value: "opacity", label: "Opacity" },
        { value: "hue", label: "Hue" },
        { value: "scheme", label: "Scheme" },
      ],
    },
    default: D.gradientMode,
  },
  {
    id: "custom.showPoints",
    label: "Show points",
    group: STYLES,
    scope: "fieldConfig",
    views: TS,
    control: {
      kind: "select",
      choices: [
        { value: "auto", label: "Auto" },
        { value: "always", label: "Always" },
        { value: "never", label: "Never" },
      ],
    },
    default: D.showPoints,
  },
  {
    id: "custom.spanNulls",
    label: "Connect null values",
    group: STYLES,
    scope: "fieldConfig",
    views: TS,
    control: { kind: "toggle" },
    default: D.spanNulls,
    keywords: ["gap", "null", "connect"],
  },
  {
    id: "custom.axisPlacement",
    label: "Placement",
    group: AXIS,
    scope: "fieldConfig",
    views: TS,
    control: {
      kind: "select",
      choices: [
        { value: "auto", label: "Auto" },
        { value: "left", label: "Left" },
        { value: "right", label: "Right" },
        { value: "hidden", label: "Hidden" },
      ],
    },
    default: D.axisPlacement,
    keywords: ["axis", "y-axis", "left", "right"],
  },
];
