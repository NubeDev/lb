// The PURE mapping `framesToOption(frames, fieldConfig) → EChartsOption`. This is the piece that makes
// the shared Field-tab options actually DRIVE the chart: it reads `fieldConfig.defaults` (unit,
// decimals, thresholds, custom.drawStyle, custom.showLegend) and shapes an ECharts option from the
// lb-viz Frame[] the shell pushes in. No React, no ECharts instance, no side effects — a pure function,
// so it is trivially testable and cannot drift into fetching (the tile never fetches; rule 3/9).
//
// Mapping rules (honest, minimal):
//   • X axis  = the FIRST time-typed field, else the first field, of the primary (first) frame.
//   • Series  = every OTHER numeric field, across all frames — one line (default) or bar series each.
//   • Labels  = `unit`/`decimals` from `fieldConfig.defaults` format the y-axis + tooltip values.
//   • Legend  = shown when >1 series, or when `custom.showLegend === true`.
//   • Thresholds → y-axis `markLine`s at each finite step (the Field-tab thresholds made visible).

import type { EChartsOption, LineSeriesOption, BarSeriesOption } from "echarts";
import type { Frame, Field, FieldConfig, FieldOptions } from "./frame.types";
import type { ChartTheme } from "./mountChart";

/** Is this field's declared type or its values numeric? (lb-viz may omit `type`.) */
function isNumericField(f: Field): boolean {
  if (f.type === "number") return true;
  if (f.type === "time" || f.type === "string") return false;
  // Untyped: sniff the first non-null value.
  const first = f.values.find((v) => v != null);
  return typeof first === "number";
}

/** Is this the X (category/time) axis field? A `time` type wins; else the first field is the axis. */
function isAxisField(f: Field, index: number): boolean {
  return f.type === "time" || index === 0;
}

/** Pretty-print a numeric value with the Field-tab `decimals` + `unit` suffix (a minimal local render;
 *  the shell's full unit engine is not importable here). */
function formatValue(v: unknown, defaults: FieldOptions): string {
  if (typeof v !== "number" || !Number.isFinite(v)) return String(v ?? "");
  const n = defaults.decimals != null ? v.toFixed(defaults.decimals) : String(v);
  return defaults.unit ? `${n} ${defaults.unit}` : n;
}

/** The `markLine` data for the Field-tab thresholds (each finite step becomes a horizontal guide). */
function thresholdMarkLines(defaults: FieldOptions) {
  const steps = defaults.thresholds?.steps ?? [];
  const finite = steps.filter((s) => typeof s.value === "number" && Number.isFinite(s.value));
  if (!finite.length) return undefined;
  return {
    silent: true,
    symbol: "none" as const,
    data: finite.map((s) => ({ yAxis: s.value as number, lineStyle: { color: s.color } })),
  };
}

/** Map the lb-viz frames + field-config (+ optional resolved theme) into an ECharts option. Pure; safe on
 *  empty/degenerate input (an empty frame set → an option with no series, which the tile treats as "no
 *  data"). When `theme` is present (ctx v4) the chart recolors — series from `theme.chart`, axis/text from
 *  `theme.fg`/`muted`/`border` — so an extension chart matches the core charts on every theme change. */
export function framesToOption(frames: Frame[], fieldConfig?: FieldConfig, theme?: ChartTheme): EChartsOption {
  const defaults: FieldOptions = fieldConfig?.defaults ?? {};
  const drawStyle = defaults.custom?.drawStyle === "bar" ? "bar" : "line";
  const markLine = thresholdMarkLines(defaults);

  // X axis from the primary frame's axis field (time-typed, else first). Its values become the category
  // axis; a `time` field renders its raw values (epoch/label) — the tile does not reformat time here.
  const primary = frames[0];
  const axisField = primary?.fields.find((f, i) => isAxisField(f, i));
  const categories = axisField ? axisField.values.map((v) => (v == null ? "" : String(v))) : [];

  // One series per numeric NON-axis field, across every frame. The series name prefers the frame's
  // refId/name so multi-frame results stay legible; falls back to the field name.
  const series: Array<LineSeriesOption | BarSeriesOption> = [];
  for (const frame of frames) {
    frame.fields.forEach((f, i) => {
      if (isAxisField(f, i) && f === axisField) return; // skip the chosen x field
      if (!isNumericField(f)) return; // string/label fields are not plotted (honest: no fake series)
      const prefix = frame.refId ?? frame.name;
      const name = prefix && frames.length > 1 ? `${prefix} · ${f.name}` : f.name;
      const data = f.values.map((v) => (typeof v === "number" ? v : null));
      if (drawStyle === "bar") {
        series.push({ type: "bar", name, data, markLine });
      } else {
        series.push({ type: "line", name, data, showSymbol: false, markLine });
      }
    });
  }

  const showLegend = defaults.custom?.showLegend === true || series.length > 1;

  // Theme-derived colors (ctx v4). Absent theme → ECharts defaults (a v3 host / standalone dev build).
  const ramp = theme?.chart && theme.chart.length ? theme.chart : undefined;
  const single = ramp && series.length <= 1 ? [theme?.accent ?? ramp[0]] : ramp;
  const textColor = theme?.fg;
  const mutedColor = theme?.muted ?? theme?.fg;
  const lineColor = theme?.border;

  return {
    color: single,
    grid: { left: 48, right: 16, top: showLegend ? 32 : 12, bottom: 28 },
    tooltip: {
      trigger: "axis",
      valueFormatter: (v) => formatValue(v, defaults),
      backgroundColor: theme?.panel,
      borderColor: lineColor,
      textStyle: textColor ? { color: textColor } : undefined,
    },
    legend: showLegend
      ? { show: true, top: 4, textStyle: { fontSize: 10, color: mutedColor } }
      : undefined,
    xAxis: {
      type: "category",
      data: categories,
      axisLabel: { fontSize: 10, color: mutedColor },
      axisLine: lineColor ? { lineStyle: { color: lineColor } } : undefined,
    },
    yAxis: {
      type: "value",
      min: defaults.min,
      max: defaults.max,
      name: defaults.displayName ?? defaults.unit,
      nameTextStyle: { fontSize: 10, color: mutedColor },
      axisLabel: {
        fontSize: 10,
        color: mutedColor,
        formatter: (v: number) => formatValue(v, defaults),
      },
      splitLine: lineColor ? { lineStyle: { color: lineColor, opacity: 0.4 } } : undefined,
    },
    series,
  };
}
