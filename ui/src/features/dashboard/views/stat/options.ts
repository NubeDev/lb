// The `stat` per-viz options (viz chart-types scope; names + defaults VERBATIM from Grafana's
// `public/app/plugins/panel/stat/panelcfg.cue` + `common.SingleStatBaseOptions`). Per-VIZ only — how
// the big-value draws (graph/color/justify/text mode + the reduceOptions frame→value bridge +
// orientation). Per-FIELD reads (unit/decimals/thresholds/color) are `fieldConfig`, not here.
//
// One responsibility: the typed `stat` options + their Grafana defaults. The renderer reads this.

import { type ReduceDataOptions, defaultReduceOptions, readReduceOptions } from "../reduce";

export type BigValueGraphMode = "none" | "line" | "area";
export type BigValueColorMode = "value" | "background" | "background_solid" | "none";
export type BigValueJustifyMode = "auto" | "center";
export type BigValueTextMode = "auto" | "value" | "value_and_name" | "name" | "none";
export type VizOrientation = "auto" | "horizontal" | "vertical";

/** Grafana's `stat` Options (the Phase-2 subset of the panelcfg — graph/color/justify/text mode,
 *  reduceOptions, orientation, showPercentChange). */
export interface StatOptions {
  reduceOptions: ReduceDataOptions;
  orientation: VizOrientation;
  graphMode: BigValueGraphMode;
  colorMode: BigValueColorMode;
  justifyMode: BigValueJustifyMode;
  textMode: BigValueTextMode;
  showPercentChange: boolean;
}

/** Grafana's out-of-the-box `stat` defaults (panelcfg `*` markers): graphMode "area", colorMode
 *  "value", justify/text "auto", orientation "auto", reduceOptions `{calcs:[]}` (→ lastNotNull). */
export function defaultStatOptions(): StatOptions {
  return {
    reduceOptions: defaultReduceOptions(),
    orientation: "auto",
    graphMode: "area",
    colorMode: "value",
    justifyMode: "auto",
    textMode: "auto",
    showPercentChange: false,
  };
}

/** Read a cell's `options` as `StatOptions`, filling any missing key with the Grafana default. */
export function readStatOptions(options: Record<string, unknown> | undefined): StatOptions {
  const d = defaultStatOptions();
  const o = (options ?? {}) as Partial<StatOptions>;
  return {
    reduceOptions: readReduceOptions(o.reduceOptions),
    orientation: o.orientation ?? d.orientation,
    graphMode: o.graphMode ?? d.graphMode,
    colorMode: o.colorMode ?? d.colorMode,
    justifyMode: o.justifyMode ?? d.justifyMode,
    textMode: o.textMode ?? d.textMode,
    showPercentChange: o.showPercentChange ?? d.showPercentChange,
  };
}
