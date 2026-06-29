// The `gauge` per-viz options (viz chart-types scope; names + defaults VERBATIM from Grafana's
// `public/app/plugins/panel/gauge/panelcfg.cue` + `common.SingleStatBaseOptions`). Per-VIZ only — the
// threshold labels/markers, orientation, sizing, the reduceOptions frame→value bridge. The arc's
// min/max + threshold STEPS are `fieldConfig` (per-field), not here.
//
// One responsibility: the typed `gauge` options + their Grafana defaults.

import { type ReduceDataOptions, defaultReduceOptions, readReduceOptions } from "../reduce";
import type { VizOrientation } from "../stat/options";

export type { VizOrientation };
export type BarGaugeSizing = "auto" | "manual";

/** Grafana's `gauge` Options (Phase-2 subset: threshold labels/markers, orientation, sizing, viz min). */
export interface GaugeOptions {
  reduceOptions: ReduceDataOptions;
  orientation: VizOrientation;
  showThresholdLabels: boolean;
  showThresholdMarkers: boolean;
  sizing: BarGaugeSizing;
  minVizWidth: number;
  minVizHeight: number;
}

/** Grafana defaults (panelcfg `*`): markers on, labels off, orientation auto, sizing auto, min 75/75. */
export function defaultGaugeOptions(): GaugeOptions {
  return {
    reduceOptions: defaultReduceOptions(),
    orientation: "auto",
    showThresholdLabels: false,
    showThresholdMarkers: true,
    sizing: "auto",
    minVizWidth: 75,
    minVizHeight: 75,
  };
}

export function readGaugeOptions(options: Record<string, unknown> | undefined): GaugeOptions {
  const d = defaultGaugeOptions();
  const o = (options ?? {}) as Partial<GaugeOptions>;
  return {
    reduceOptions: readReduceOptions(o.reduceOptions),
    orientation: o.orientation ?? d.orientation,
    showThresholdLabels: o.showThresholdLabels ?? d.showThresholdLabels,
    showThresholdMarkers: o.showThresholdMarkers ?? d.showThresholdMarkers,
    sizing: o.sizing ?? d.sizing,
    minVizWidth: o.minVizWidth ?? d.minVizWidth,
    minVizHeight: o.minVizHeight ?? d.minVizHeight,
  };
}
