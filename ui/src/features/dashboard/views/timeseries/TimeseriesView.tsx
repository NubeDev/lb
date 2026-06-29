// The v3 `timeseries` panel renderer (viz chart-types scope, Phase 1 — "one chart end to end"). It
// replaces the bad single-`unit`-string chart with the full Grafana surface: per-viz options
// (legend/tooltip via `options.ts`), per-field draw options (drawStyle/lineWidth/fillOpacity via the
// `custom` bag), and the `fieldConfig` render path (unit/decimals/min-max/thresholds/color) — all values
// formatted through the ONE user-prefs bridge (`fieldconfig/format.ts`), never a local `toFixed`/unit
// string. Data comes ONLY through `usePanelData` (invariant A: the single swap point for Phase-3
// `viz.query`); there is no client-side transform (invariant B).
//
// One responsibility: render a timeseries panel from a cell. Data access, option shapes, the chart SVG,
// the legend, and the fieldConfig math each live in their own file; this composes them.

import type { Cell } from "@/lib/dashboard";
import { cellFieldConfig } from "@/lib/dashboard";
import type { VarScope } from "@/lib/vars";
import { emptyScope } from "@/lib/vars";

import { WidgetHeader, WidgetMessage } from "../../widgets/chrome";
import { TimeseriesChart } from "../../widgets/recharts";
import { usePanelData } from "../../builder/usePanelData";
import { rowNumber } from "../num";
import { readTimeseriesOptions } from "./options";
import { readTimeseriesCustom } from "./custom";
import { TimeseriesLegend } from "./Legend";
import { resolveFieldOptions } from "../../fieldconfig/resolve";
import { formatValue } from "../../fieldconfig/format";
import { thresholdColor } from "../../fieldconfig/thresholds";
import { resolveFieldColor } from "../../fieldconfig/color";

interface Props {
  /** The whole cell — the renderer reads `sources`/`options`/`fieldConfig` from it (v3) or falls back
   *  to the v2 `source`/`options` via the adapters. */
  cell: Cell;
  label?: string;
  scope?: VarScope;
  refreshKey?: number;
}

/** Bounded backfill so a min/max domain stays meaningful (mirrors the v2 chart). */
const FIELD_NAME = "value";

export function TimeseriesView({ cell, label, scope = emptyScope(), refreshKey = 0 }: Props) {
  const { rows, loading, denied } = usePanelData(cell, scope, refreshKey);

  if (denied) return <WidgetMessage tone="denied">no access to this source</WidgetMessage>;
  if (loading) return <WidgetMessage tone="muted">loading…</WidgetMessage>;

  const points = rows.map(rowNumber).filter((n): n is number => n !== null);
  if (points.length === 0) return <WidgetMessage tone="muted">no data yet</WidgetMessage>;

  // Per-viz options (legend/tooltip) — Grafana defaults filled for any missing key.
  const options = readTimeseriesOptions(cell.options);
  // The effective per-field options for the value field = fieldConfig.defaults + matching overrides.
  const fc = cellFieldConfig(cell);
  const opts = resolveFieldOptions(fc, { name: FIELD_NAME, type: "number" });
  const custom = readTimeseriesCustom(opts.custom);

  const last = points[points.length - 1];
  // The line color: thresholds color by the LAST canonical value (against canonical bounds), else the
  // field's color mode, else the accent token. Bounds are canonical (field-config scope, Risks).
  const baseColor = resolveFieldColor(opts.color);
  const color =
    opts.thresholds && opts.thresholds.steps.length > 0
      ? thresholdColor(last, opts.thresholds, baseColor, { min: opts.min, max: opts.max })
      : baseColor;

  // The Y domain: explicit min/max (canonical) when set, else padded from the data.
  const dataMin = Math.min(...points);
  const dataMax = Math.max(...points);
  const domain: [number, number] = [
    opts.min ?? dataMin - pad(dataMin, dataMax),
    opts.max ?? dataMax + pad(dataMin, dataMax),
  ];

  // The latest readout — formatted through the ONE bridge (unit/decimals → localized string).
  const latest = formatValue(last, opts);

  return (
    <div className="flex h-full flex-col" aria-label={`timeseries ${label ?? ""}`}>
      <WidgetHeader label={label ?? ""} />
      <TimeseriesChart
        points={points}
        color={color}
        drawStyle={custom.drawStyle}
        fillOpacity={custom.fillOpacity}
        lineWidth={custom.lineWidth}
        showTooltip={options.tooltip.mode !== "none"}
        domain={domain}
        valueFormatter={(v) => formatValue(v, opts).text}
        ariaLabel="timeseries line"
      />
      <div className="mt-1 text-right text-xs text-muted tabular-nums" aria-label="timeseries latest">
        {opts.noValue && !Number.isFinite(last) ? opts.noValue : latest.text}
      </div>
      <TimeseriesLegend
        legend={options.legend}
        seriesName={opts.displayName || label || FIELD_NAME}
        color={color}
        points={points}
        opts={opts}
      />
    </div>
  );
}

function pad(min: number, max: number): number {
  const span = max - min;
  return span === 0 ? Math.max(Math.abs(max) * 0.05, 1) : span * 0.14;
}
