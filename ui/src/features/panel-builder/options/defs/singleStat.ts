// The single-stat family PER-VIZ value options (editor-parity scope, step 5) — stat / gauge / bargauge
// / piechart. `options.*` scope, names + defaults verbatim from Grafana's panelcfgs +
// `SingleStatBaseOptions`. The reduceOptions calc bridge keeps its bespoke editor; the value-display
// options (orientation, color mode, text mode, threshold labels/markers, display mode, pie type) come
// from here through OptionGroups. One responsibility: the single-stat-family per-viz option catalog.

import type { OptionDef } from "../types";
import type { View } from "@/lib/dashboard";

const STAT: View[] = ["stat"];
const GAUGE: View[] = ["gauge"];
const BARGAUGE: View[] = ["bargauge"];
const PIE: View[] = ["piechart"];
const VALUE = "Value options";
const DISPLAY = "Display";

const orientation = (views: View[]): OptionDef => ({
  id: "orientation",
  label: "Orientation",
  group: DISPLAY,
  scope: "options",
  views,
  control: { kind: "select", choices: [{ value: "auto", label: "Auto" }, { value: "horizontal", label: "Horizontal" }, { value: "vertical", label: "Vertical" }] },
  default: "auto",
});

export const SINGLE_STAT_OPTIONS: OptionDef[] = [
  // --- stat ---
  {
    id: "colorMode",
    label: "Color mode",
    group: VALUE,
    scope: "options",
    views: STAT,
    control: { kind: "select", choices: [{ value: "value", label: "Value" }, { value: "background", label: "Background" }, { value: "background_solid", label: "Background solid" }, { value: "none", label: "None" }] },
    default: "value",
  },
  {
    id: "graphMode",
    label: "Graph mode",
    group: VALUE,
    scope: "options",
    views: STAT,
    control: { kind: "select", choices: [{ value: "none", label: "None" }, { value: "area", label: "Area" }, { value: "line", label: "Line" }] },
    default: "area",
  },
  {
    id: "textMode",
    label: "Text mode",
    group: VALUE,
    scope: "options",
    views: STAT,
    control: { kind: "select", choices: [{ value: "auto" }, { value: "value" }, { value: "value_and_name" }, { value: "name" }, { value: "none" }] },
    default: "auto",
  },
  {
    id: "justifyMode",
    label: "Justify",
    group: VALUE,
    scope: "options",
    views: STAT,
    control: { kind: "select", choices: [{ value: "auto" }, { value: "center" }] },
    default: "auto",
  },
  { id: "showPercentChange", label: "Show percent change", group: VALUE, scope: "options", views: STAT, control: { kind: "toggle" }, default: false },
  orientation(STAT),

  // --- gauge ---
  { id: "showThresholdLabels", label: "Show threshold labels", group: VALUE, scope: "options", views: GAUGE, control: { kind: "toggle" }, default: false },
  { id: "showThresholdMarkers", label: "Show threshold markers", group: VALUE, scope: "options", views: GAUGE, control: { kind: "toggle" }, default: true },
  orientation(GAUGE),

  // --- bargauge ---
  {
    id: "displayMode",
    label: "Display mode",
    group: VALUE,
    scope: "options",
    views: BARGAUGE,
    control: { kind: "select", choices: [{ value: "gradient", label: "Gradient" }, { value: "lcd", label: "Retro LCD" }, { value: "basic", label: "Basic" }] },
    default: "gradient",
  },
  { id: "showUnfilled", label: "Show unfilled area", group: VALUE, scope: "options", views: BARGAUGE, control: { kind: "toggle" }, default: true },
  orientation(BARGAUGE),

  // --- piechart ---
  {
    id: "pieType",
    label: "Pie type",
    group: VALUE,
    scope: "options",
    views: PIE,
    control: { kind: "select", choices: [{ value: "pie", label: "Pie" }, { value: "donut", label: "Donut" }] },
    default: "pie",
  },
  // NOTE: pie `displayLabels` is a `PieChartLabel[]` array in the render path — not a scalar select — so
  // it is intentionally NOT a registry option (it would round-trip wrong). It stays in the bespoke pie
  // editor as a multi-toggle. Named, not a silent gap (scope non-goal for the registry).
];
