// The React chart tile — a PURE RENDERER of frames. It takes `{ frames, fieldConfig }` (never a bridge,
// never a token, never a fetch) and draws an Apache ECharts chart from `framesToOption`. This is the
// reference "data tile": everything it shows comes from `ctx.data` the shell pushed in, so the SAME
// render path works on any surface that can hand a tile frames (dashboard cells, channel surfaces).
//
// Honest states (never a fabricated series):
//   • no frames / all-empty  → "no data";
//   • a frame that looks like an error (`{ error }` / an `error` field) → render its message;
//   • otherwise → the chart.
// The ECharts instance is created in a ref'd div, `setOption` re-applied whenever frames/fieldConfig
// change, and DISPOSED on unmount (stateless eviction — no lingering canvas/observer).

import { useEffect, useRef } from "react";
import * as echarts from "echarts";

import type { Frame, FieldConfig } from "./frame.types";
import { framesToOption } from "./framesToOption";
import type { ChartTheme } from "./mountChart";

interface Props {
  frames: Frame[];
  fieldConfig?: FieldConfig;
  /** v4 resolved theme tokens — the chart recolors from these on a theme change (ECharts can't read a
   *  CSS var). Absent on a v3 host → ECharts defaults. */
  theme?: ChartTheme;
}

/** True when there is nothing plottable — no frames, or every frame has no rows. Honest empty, no fake. */
function isEmpty(frames: Frame[]): boolean {
  if (!frames.length) return true;
  return frames.every((f) => f.fields.every((fld) => fld.values.length === 0));
}

/** Pull an error message out of a frame shaped like an error, if any. The shell may push an error frame
 *  (a denied/failed source) instead of data; we surface it rather than draw an empty chart as if fine. */
function errorMessage(frames: Frame[]): string | null {
  for (const f of frames) {
    const asErr = f as unknown as { error?: unknown };
    if (typeof asErr.error === "string" && asErr.error) return asErr.error;
    const errField = f.fields.find((fld) => fld.name === "error");
    const first = errField?.values.find((v) => typeof v === "string" && v);
    if (typeof first === "string") return first;
  }
  return null;
}

/** A frames-in ECharts tile. Re-renders (setOption) on every fresh `frames`/`fieldConfig`; disposes on
 *  unmount. `notMerge: true` so a shrinking series set does not leave stale series behind. */
export function ChartTile({ frames, fieldConfig, theme }: Props) {
  const divRef = useRef<HTMLDivElement | null>(null);
  const chartRef = useRef<echarts.ECharts | null>(null);

  const error = errorMessage(frames);
  const empty = !error && isEmpty(frames);
  // Re-key the effect on the theme so a live theme change re-applies the option with the new colors.
  const themeKey = JSON.stringify(theme ?? null);

  useEffect(() => {
    // Nothing to draw in the error/empty branches — tear any existing chart down so the honest message
    // is not painted over a stale canvas.
    if (error || empty) {
      chartRef.current?.dispose();
      chartRef.current = null;
      return;
    }
    if (!divRef.current) return;
    if (!chartRef.current) {
      chartRef.current = echarts.init(divRef.current);
    }
    chartRef.current.setOption(framesToOption(frames, fieldConfig, theme), { notMerge: true });
    // eslint-disable-next-line react-hooks/exhaustive-deps -- themeKey stands in for the theme object dep
  }, [frames, fieldConfig, error, empty, themeKey]);

  // Resize the chart with its container (the dashboard cell is resizable). Cheap ResizeObserver, torn
  // down with the tile; disposes the instance on final unmount.
  useEffect(() => {
    const el = divRef.current;
    if (!el || typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(() => chartRef.current?.resize());
    ro.observe(el);
    return () => {
      ro.disconnect();
      chartRef.current?.dispose();
      chartRef.current = null;
    };
  }, []);

  if (error) {
    return (
      <div className="flex h-full w-full items-center justify-center p-2" data-echarts-state="error">
        <span className="text-xs text-red-400">{error}</span>
      </div>
    );
  }
  if (empty) {
    return (
      <div className="flex h-full w-full items-center justify-center p-2" data-echarts-state="empty">
        <span className="text-xs text-muted">no data</span>
      </div>
    );
  }
  return (
    <div
      ref={divRef}
      className="h-full w-full"
      data-echarts-state="chart"
      aria-label="echarts chart tile"
    />
  );
}
