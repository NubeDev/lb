// The `timeseries` PER-VIZ options (editor-parity scope, step 5) — legend + tooltip + stacking +
// threshold display style, `options.*` scope, names verbatim from Grafana's timeseries panelcfg. The
// legend `calcs` multi-toggle keeps its bespoke editor in TimeseriesOptionsEditor (a chip row); the
// simple legend/tooltip/stacking options come from here through OptionGroups. Stacking mode is a
// per-field `custom.stacking.mode` in Grafana; we expose the panel-level default here. One
// responsibility: the timeseries per-viz option catalog.

import type { OptionDef } from "../types";

const TS = ["timeseries" as const];
const LEGEND = "Legend";
const TOOLTIP = "Tooltip";
const GRAPH = "Graph styles";

export const TIMESERIES_VIZ_OPTIONS: OptionDef[] = [
  { id: "legend.showLegend", label: "Show legend", group: LEGEND, scope: "options", path: "legend.showLegend", views: TS, control: { kind: "toggle" }, default: true },
  {
    id: "legend.displayMode",
    label: "Legend mode",
    group: LEGEND,
    scope: "options",
    path: "legend.displayMode",
    views: TS,
    control: { kind: "select", choices: [{ value: "list", label: "List" }, { value: "table", label: "Table" }, { value: "hidden", label: "Hidden" }] },
    default: "list",
  },
  {
    id: "legend.placement",
    label: "Legend placement",
    group: LEGEND,
    scope: "options",
    path: "legend.placement",
    views: TS,
    control: { kind: "select", choices: [{ value: "bottom", label: "Bottom" }, { value: "right", label: "Right" }] },
    default: "bottom",
  },
  {
    id: "tooltip.mode",
    label: "Tooltip mode",
    group: TOOLTIP,
    scope: "options",
    path: "tooltip.mode",
    views: TS,
    control: { kind: "select", choices: [{ value: "single", label: "Single" }, { value: "multi", label: "All" }, { value: "none", label: "Hidden" }] },
    default: "single",
  },
  // Stacking (Grafana's per-field `custom.stacking.mode`; the panel default sits in fieldConfig custom).
  {
    id: "custom.stacking.mode",
    label: "Stacking",
    group: GRAPH,
    scope: "fieldConfig",
    path: "custom.stacking.mode",
    views: TS,
    control: { kind: "select", choices: [{ value: "none", label: "Off" }, { value: "normal", label: "Normal" }, { value: "percent", label: "100%" }] },
    default: "none",
    keywords: ["stack", "area"],
  },
  // Threshold display style (Grafana's `custom.thresholdsStyle.mode`).
  {
    id: "custom.thresholdsStyle.mode",
    label: "Show thresholds",
    group: GRAPH,
    scope: "fieldConfig",
    path: "custom.thresholdsStyle.mode",
    views: TS,
    control: {
      kind: "select",
      choices: [
        { value: "off", label: "Off" },
        { value: "line", label: "As lines" },
        { value: "area", label: "As filled regions" },
        { value: "line+area", label: "Lines + regions" },
      ],
    },
    default: "off",
    keywords: ["threshold", "line", "region"],
  },
];
