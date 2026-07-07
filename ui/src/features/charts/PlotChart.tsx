// THE shared 10x chart renderer — one Recharts SVG for BOTH surfaces (dashboard panels + in-channel
// query results). Give it rows + a `PlotSpec` and it draws a real chart with the things the old
// hand-drawn built-ins dropped: titled, ticked X and Y axes; faint dashed gridlines; a themed rich
// tooltip; a legend for multi-series; a reduced-motion-aware draw-in; and honest empty/table-only
// states. Data shaping is `buildPlot` (the ONE transform), styling is `chartTheme`; this file only
// composes chart elements per `spec.type`, so the two surfaces can never drift.
//
// One responsibility: render a chart from (rows, spec). It does NOT fetch, pick, or persist.

import { useMemo } from "react";
import {
  Area,
  AreaChart,
  Bar,
  BarChart,
  CartesianGrid,
  Cell as PieCell,
  Legend,
  Line,
  LineChart,
  Pie,
  PieChart,
  ResponsiveContainer,
  Scatter,
  ScatterChart,
  Tooltip,
  XAxis,
  YAxis,
  ZAxis,
} from "recharts";

import { buildPlot, isPlottable, type PlotSpec } from "@/lib/charts";
import { axisLabelStyle, axisLine, axisTick, categoryColors, gridProps, seriesColor } from "./chartTheme";
import { ChartTooltip } from "./ChartTooltip";
import { ChartState } from "./ChartStates";
import { formatXTick, isTemporalAxis } from "./axisFormat";
import { useReducedMotion } from "./useReducedMotion";
import { downsampleRows } from "./downsample";
import { useChartBudget } from "./chartBudget";

interface Props {
  rows: Array<Record<string, unknown>>;
  spec: PlotSpec;
  /** Axis titles — default to the spec's field names so an axis is never unlabeled. */
  xLabel?: string;
  yLabel?: string;
  /** Localise y/tooltip numbers (dashboard passes its user-prefs bridge; channel a plain formatter). */
  valueFormatter?: (n: number) => string;
  ariaLabel?: string;
}

const MARGIN = { top: 12, right: 18, bottom: 26, left: 8 } as const;
const defaultFormat = (n: number) => (Number.isInteger(n) ? n.toLocaleString() : n.toLocaleString(undefined, { maximumFractionDigits: 3 }));

export function PlotChart({ rows, spec, xLabel, yLabel, valueFormatter = defaultFormat, ariaLabel }: Props) {
  const reduced = useReducedMotion();
  const budget = useChartBudget();
  // Downsample what is DRAWN (shared-x representative rows — see downsample.ts) AFTER the full-data
  // aggregation: buildPlot's grouping/reduction still sees every row, only the plotted frame shrinks.
  // EVERY series is always drawn (never hide data) — a high-cardinality split instead shrinks the
  // per-series point budget so the TOTAL drawn points stay bounded (600 series can't mean 600×1500
  // SVG points), floored so each series still has a visible shape.
  const frame = useMemo(() => {
    const f = buildPlot(rows, spec);
    const rowBudget = Math.max(50, Math.floor((budget * 8) / Math.max(1, f.series.length)));
    const cap = Math.min(budget, rowBudget);
    return f.data.length > cap ? { ...f, data: downsampleRows(f.data, cap) } : f;
  }, [rows, spec, budget]);

  if (!isPlottable(spec)) return <ChartState tone="table-only" />;
  if (frame.data.length === 0) return <ChartState tone="empty" />;

  const colorFor = (key: string) => {
    const i = frame.series.findIndex((s) => s.key === key);
    return seriesColor(Math.max(i, 0), frame.series.length);
  };
  const animate = !reduced;
  const temporal = isTemporalAxis(frame.data[0]?.[frame.xKey]);
  const xTitle = xLabel ?? spec.xField;
  const yTitle = yLabel ?? spec.yFields.join(" · ");

  // --- pie: no cartesian axes; one slice per aggregated category ---
  if (spec.type === "pie") {
    const colors = categoryColors(frame.data.length);
    return (
      <Wrap ariaLabel={ariaLabel ?? "pie chart"}>
        <PieChart margin={{ top: 8, right: 8, bottom: 8, left: 8 }}>
          <Tooltip content={<ChartTooltip valueFormatter={valueFormatter} colorOf={() => "hsl(var(--accent))"} />} />
          <Legend wrapperStyle={{ fontSize: 11, maxHeight: 88, overflowY: "auto" }} />
          <Pie
            data={frame.data}
            dataKey="value"
            nameKey={frame.xKey}
            cx="50%"
            cy="50%"
            outerRadius="78%"
            innerRadius="0%"
            paddingAngle={1}
            isAnimationActive={animate}
            stroke="hsl(var(--panel))"
            strokeWidth={2}
          >
            {frame.data.map((_, i) => (
              <PieCell key={i} fill={colors[i % colors.length]} />
            ))}
          </Pie>
        </PieChart>
      </Wrap>
    );
  }

  const grid = <CartesianGrid {...gridProps} vertical={spec.type === "bar" && !spec.horizontal ? false : true} />;
  const tooltip = <Tooltip cursor={{ fill: "hsl(var(--accent) / 0.06)" }} content={<ChartTooltip valueFormatter={valueFormatter} colorOf={colorFor} />} />;
  // The legend is CONTAINED, whatever the series count: past ~4 rows of entries it scrolls inside the
  // pane instead of painting over the page (the 600-series "split by point_id" failure). Every series
  // keeps its entry — bounding the box, not the data.
  const legend =
    frame.series.length > 1 ? (
      <Legend wrapperStyle={{ fontSize: 11, paddingTop: 4, maxHeight: 88, overflowY: "auto" }} iconType="plainline" />
    ) : null;

  const xAxis = (
    <XAxis
      dataKey={frame.xKey}
      tick={axisTick}
      tickLine={false}
      axisLine={axisLine}
      tickFormatter={formatXTick}
      minTickGap={temporal ? 28 : 8}
      interval="preserveStartEnd"
      height={xTitle ? 40 : 24}
      label={xTitle ? { value: xTitle, position: "insideBottom", offset: 0, style: axisLabelStyle } : undefined}
    />
  );
  const yAxis = (
    <YAxis
      tick={axisTick}
      tickLine={false}
      axisLine={false}
      width={54}
      tickFormatter={(v: number) => valueFormatter(Number(v))}
      label={yTitle ? { value: yTitle, angle: -90, position: "insideLeft", offset: 12, style: { ...axisLabelStyle, textAnchor: "middle" } } : undefined}
    />
  );

  if (spec.type === "bar") {
    return (
      <Wrap ariaLabel={ariaLabel ?? "bar chart"}>
        <BarChart data={frame.data} layout={spec.horizontal ? "vertical" : "horizontal"} margin={MARGIN} barCategoryGap="18%">
          {grid}
          {spec.horizontal ? (
            <>
              <YAxis type="category" dataKey={frame.xKey} tick={axisTick} tickLine={false} axisLine={axisLine} width={80} tickFormatter={formatXTick} />
              <XAxis type="number" tick={axisTick} tickLine={false} axisLine={false} tickFormatter={(v: number) => valueFormatter(Number(v))} height={xTitle ? 40 : 24} label={xTitle ? { value: yTitle, position: "insideBottom", offset: 0, style: axisLabelStyle } : undefined} />
            </>
          ) : (
            <>
              {xAxis}
              {yAxis}
            </>
          )}
          {tooltip}
          {legend}
          {frame.series.map((s, i) => (
            <Bar key={s.key} dataKey={s.key} name={s.name} stackId={spec.stacked ? "s" : undefined} fill={seriesColor(i, frame.series.length)} radius={spec.stacked ? 0 : [4, 4, 0, 0]} isAnimationActive={animate} maxBarSize={64} />
          ))}
        </BarChart>
      </Wrap>
    );
  }

  if (spec.type === "scatter") {
    return (
      <Wrap ariaLabel={ariaLabel ?? "scatter chart"}>
        <ScatterChart margin={MARGIN}>
          {grid}
          {xAxis}
          {yAxis}
          <ZAxis range={[36, 36]} />
          {tooltip}
          {legend}
          {frame.series.map((s, i) => (
            <Scatter key={s.key} dataKey={s.key} name={s.name} data={frame.data} fill={seriesColor(i, frame.series.length)} isAnimationActive={animate} />
          ))}
        </ScatterChart>
      </Wrap>
    );
  }

  if (spec.type === "area") {
    return (
      <Wrap ariaLabel={ariaLabel ?? "area chart"}>
        <AreaChart data={frame.data} margin={MARGIN}>
          <defs>
            {frame.series.map((s, i) => {
              const c = seriesColor(i, frame.series.length);
              return (
                <linearGradient key={s.key} id={`fill-${s.key}`} x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0%" stopColor={c} stopOpacity={0.4} />
                  <stop offset="100%" stopColor={c} stopOpacity={0.04} />
                </linearGradient>
              );
            })}
          </defs>
          {grid}
          {xAxis}
          {yAxis}
          {tooltip}
          {legend}
          {frame.series.map((s, i) => (
            <Area key={s.key} type={spec.smooth ? "monotone" : "linear"} dataKey={s.key} name={s.name} stackId={spec.stacked ? "s" : undefined} stroke={seriesColor(i, frame.series.length)} strokeWidth={2} fill={`url(#fill-${s.key})`} dot={false} activeDot={{ r: 3.5 }} isAnimationActive={animate} connectNulls />
          ))}
        </AreaChart>
      </Wrap>
    );
  }

  // line (default)
  return (
    <Wrap ariaLabel={ariaLabel ?? "line chart"}>
      <LineChart data={frame.data} margin={MARGIN}>
        {grid}
        {xAxis}
        {yAxis}
        {tooltip}
        {legend}
        {frame.series.map((s, i) => (
          <Line key={s.key} type={spec.smooth ? "monotone" : "linear"} dataKey={s.key} name={s.name} stroke={seriesColor(i, frame.series.length)} strokeWidth={2} dot={false} activeDot={{ r: 3.5 }} isAnimationActive={animate} connectNulls />
        ))}
      </LineChart>
    </Wrap>
  );
}

function Wrap({ children, ariaLabel }: { children: React.ReactElement; ariaLabel: string }) {
  return (
    <div className="min-h-0 flex-1 overflow-hidden" role="img" aria-label={ariaLabel}>
      <ResponsiveContainer width="100%" height="100%" minHeight={0}>
        {children}
      </ResponsiveContainer>
    </div>
  );
}
