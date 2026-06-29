// The `bargauge` per-viz options (viz chart-types scope; names + defaults VERBATIM from Grafana's
// `public/app/plugins/panel/bargauge/panelcfg.cue` + `common.SingleStatBaseOptions`). Per-VIZ only —
// the bar display mode (basic/lcd/gradient), value mode, orientation, showUnfilled, the reduceOptions
// frame→value bridge. Min/max + threshold steps are `fieldConfig` (per-field).
//
// One responsibility: the typed `bargauge` options + their Grafana defaults.

import { type ReduceDataOptions, defaultReduceOptions, readReduceOptions } from "../reduce";
import type { VizOrientation } from "../stat/options";

export type { VizOrientation };
export type BarGaugeDisplayMode = "basic" | "lcd" | "gradient";
export type BarGaugeValueMode = "color" | "text" | "hidden";

/** Grafana's `bargauge` Options (Phase-2 subset). */
export interface BarGaugeOptions {
  reduceOptions: ReduceDataOptions;
  orientation: VizOrientation;
  displayMode: BarGaugeDisplayMode;
  valueMode: BarGaugeValueMode;
  showUnfilled: boolean;
  minVizWidth: number;
  minVizHeight: number;
}

/** Grafana defaults (panelcfg `*`): displayMode "gradient", valueMode "color", showUnfilled true. */
export function defaultBarGaugeOptions(): BarGaugeOptions {
  return {
    reduceOptions: defaultReduceOptions(),
    orientation: "auto",
    displayMode: "gradient",
    valueMode: "color",
    showUnfilled: true,
    minVizWidth: 8,
    minVizHeight: 16,
  };
}

export function readBarGaugeOptions(options: Record<string, unknown> | undefined): BarGaugeOptions {
  const d = defaultBarGaugeOptions();
  const o = (options ?? {}) as Partial<BarGaugeOptions>;
  return {
    reduceOptions: readReduceOptions(o.reduceOptions),
    orientation: o.orientation ?? d.orientation,
    displayMode: o.displayMode ?? d.displayMode,
    valueMode: o.valueMode ?? d.valueMode,
    showUnfilled: o.showUnfilled ?? d.showUnfilled,
    minVizWidth: o.minVizWidth ?? d.minVizWidth,
    minVizHeight: o.minVizHeight ?? d.minVizHeight,
  };
}
