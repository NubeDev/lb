// Shared Recharts renderers for built-in dashboard widgets. Data access stays in the v1/v2 containers;
// this file only owns the chart SVGs so the built-ins do not drift between hand-drawn implementations.

import {
  Area,
  AreaChart,
  Bar,
  BarChart,
  CartesianGrid,
  Cell as PieSlice,
  Legend as RechartsLegend,
  Line,
  LineChart,
  Pie,
  PieChart,
  PolarAngleAxis,
  RadialBar,
  RadialBarChart,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";

function paddedDomain(points: number[]): [number, number] {
  const min = Math.min(...points);
  const max = Math.max(...points);
  const span = max - min;
  const pad = span === 0 ? Math.max(Math.abs(max) * 0.05, 1) : span * 0.14;
  return [min - pad, max + pad];
}

export function SeriesLineChart({ points, ariaLabel }: { points: number[]; ariaLabel: string }) {
  const data = points.map((value, index) => ({ index, value }));
  const domain = paddedDomain(points);

  return (
    <div className="widget-no-drag pointer-events-none min-h-0 flex-1 text-accent" role="img" aria-label={ariaLabel}>
      <AreaChart
        width={320}
        height={132}
        data={data}
        margin={{ top: 12, right: 12, bottom: 10, left: 12 }}
        accessibilityLayer={false}
        style={{ width: "100%", height: "100%" }}
      >
        <CartesianGrid
          vertical={false}
          stroke="hsl(var(--border))"
          strokeDasharray="3 5"
          strokeOpacity={0.42}
        />
        <XAxis dataKey="index" hide />
        <YAxis dataKey="value" domain={domain} hide />
        <Area
          type="monotone"
          dataKey="value"
          stroke="currentColor"
          strokeWidth={2.25}
          strokeLinecap="round"
          strokeLinejoin="round"
          fill="hsl(var(--accent))"
          fillOpacity={0.12}
          dot={false}
          isAnimationActive={false}
        />
      </AreaChart>
    </div>
  );
}

/** The Grafana-style timeseries chart — draw style (line/bars/points-as-area), a resolved color (the
 *  threshold/fixed color from the field-config bridge), and an optional tooltip. The data access +
 *  fieldConfig resolution stay in `TimeseriesView`; this owns ONLY the SVG so renderers don't drift.
 *  `valueFormatter` localizes the tooltip/axis number through the one user-prefs bridge (never `toFixed`
 *  here). */
export function TimeseriesChart({
  points,
  color,
  drawStyle,
  fillOpacity,
  lineWidth,
  showTooltip,
  domain,
  valueFormatter,
  ariaLabel,
}: {
  points: number[];
  color: string;
  drawStyle: "line" | "bars" | "points";
  fillOpacity: number;
  lineWidth: number;
  showTooltip: boolean;
  domain: [number, number];
  valueFormatter: (v: number) => string;
  ariaLabel: string;
}) {
  const data = points.map((value, index) => ({ index, value }));
  const grid = (
    <CartesianGrid vertical={false} stroke="hsl(var(--border))" strokeDasharray="3 5" strokeOpacity={0.42} />
  );
  const x = <XAxis dataKey="index" hide />;
  const y = <YAxis dataKey="value" domain={domain} hide />;
  const tip = showTooltip ? (
    <Tooltip
      isAnimationActive={false}
      formatter={(v) => valueFormatter(Number(v))}
      contentStyle={{
        background: "hsl(var(--panel))",
        border: "1px solid hsl(var(--border))",
        borderRadius: 6,
        fontSize: 11,
        color: "hsl(var(--fg))",
      }}
    />
  ) : null;
  const common = {
    width: 320,
    height: 132,
    data,
    margin: { top: 12, right: 12, bottom: 10, left: 12 },
    accessibilityLayer: false,
    style: { width: "100%", height: "100%" } as const,
  };

  return (
    <div
      className="widget-no-drag min-h-0 flex-1"
      style={{ color }}
      role="img"
      aria-label={ariaLabel}
      data-draw-style={drawStyle}
    >
      {drawStyle === "bars" ? (
        <BarChart {...common}>
          {grid}
          {x}
          {y}
          {tip}
          <Bar dataKey="value" fill="currentColor" fillOpacity={Math.max(fillOpacity / 100, 0.3)} isAnimationActive={false} />
        </BarChart>
      ) : drawStyle === "points" ? (
        <LineChart {...common}>
          {grid}
          {x}
          {y}
          {tip}
          <Line
            type="monotone"
            dataKey="value"
            stroke="currentColor"
            strokeWidth={0}
            dot={{ r: 2.5, fill: "currentColor" }}
            isAnimationActive={false}
          />
        </LineChart>
      ) : (
        <AreaChart {...common}>
          {grid}
          {x}
          {y}
          {tip}
          <Area
            type="monotone"
            dataKey="value"
            stroke="currentColor"
            strokeWidth={lineWidth + 1.25}
            strokeLinecap="round"
            strokeLinejoin="round"
            fill="currentColor"
            fillOpacity={fillOpacity / 100}
            dot={false}
            isAnimationActive={false}
          />
        </AreaChart>
      )}
    </div>
  );
}

export function GaugeArcChart({
  fraction,
  ariaLabel,
  color = "hsl(var(--accent))",
}: {
  fraction: number;
  ariaLabel: string;
  /** The arc fill — the resolved threshold/fixed color from the field-config bridge (v3 gauge); defaults
   *  to the accent token for the v1/v2 gauge that passes no color. */
  color?: string;
}) {
  const data = [{ value: fraction }];

  return (
    <div className="widget-no-drag pointer-events-none h-[76px] w-full max-w-[160px]" role="img" aria-label={ariaLabel}>
      <RadialBarChart
        width={160}
        height={76}
        data={data}
        startAngle={180}
        endAngle={0}
        cx="50%"
        cy="78%"
        innerRadius="72%"
        outerRadius="100%"
        margin={{ top: 2, right: 6, bottom: 2, left: 6 }}
        accessibilityLayer={false}
        style={{ width: "100%", height: "100%" }}
      >
        <PolarAngleAxis type="number" domain={[0, 1]} tick={false} />
        <RadialBar
          dataKey="value"
          background={{ fill: "hsl(var(--border))" }}
          barSize={8}
          cornerRadius={8}
          fill={color}
          isAnimationActive={false}
        />
      </RadialBarChart>
    </div>
  );
}

/** One drawn slice/bar datum — a label, its (canonical) numeric value, the resolved color, and the
 *  display text (already formatted through the one user-prefs bridge by the caller). */
export interface CategoryDatum {
  name: string;
  value: number;
  color: string;
  text: string;
}

/** The piechart SVG (viz chart-types scope, the `piechart` `pieType`/`displayLabels`/legend). `donut`
 *  sets an inner radius. Colors + the formatted `text` are pre-resolved by the panel (this owns ONLY the
 *  SVG, so renderers don't drift). `valueFormatter` localizes the tooltip number through the one bridge. */
export function PieChartSvg({
  data,
  pieType,
  showLegend,
  showTooltip,
  valueFormatter,
  ariaLabel,
}: {
  data: CategoryDatum[];
  pieType: "pie" | "donut";
  showLegend: boolean;
  showTooltip: boolean;
  valueFormatter: (v: number) => string;
  ariaLabel: string;
}) {
  return (
    <div className="widget-no-drag min-h-0 flex-1" role="img" aria-label={ariaLabel}>
      <PieChart width={280} height={180} style={{ width: "100%", height: "100%" }}>
        {showTooltip && (
          <Tooltip
            isAnimationActive={false}
            formatter={(v) => valueFormatter(Number(v))}
            contentStyle={tooltipStyle}
          />
        )}
        {showLegend && <RechartsLegend wrapperStyle={{ fontSize: 11 }} />}
        <Pie
          data={data}
          dataKey="value"
          nameKey="name"
          cx="50%"
          cy="50%"
          outerRadius="80%"
          innerRadius={pieType === "donut" ? "55%" : 0}
          isAnimationActive={false}
        >
          {data.map((d, i) => (
            <PieSlice key={i} fill={d.color} />
          ))}
        </Pie>
      </PieChart>
    </div>
  );
}

/** The categorical bar chart (viz chart-types scope, `barchart`). One bar per category, vertical or
 *  horizontal, each colored by its resolved field color. The data/colors/format are owned by the panel;
 *  this owns ONLY the SVG. `valueFormatter` localizes the tooltip number through the one bridge. */
export function CategoryBarChart({
  data,
  horizontal,
  showTooltip,
  valueFormatter,
  ariaLabel,
}: {
  data: CategoryDatum[];
  horizontal: boolean;
  showTooltip: boolean;
  valueFormatter: (v: number) => string;
  ariaLabel: string;
}) {
  return (
    <div className="widget-no-drag min-h-0 flex-1" role="img" aria-label={ariaLabel}>
      <BarChart
        width={320}
        height={160}
        data={data}
        layout={horizontal ? "vertical" : "horizontal"}
        margin={{ top: 8, right: 12, bottom: 8, left: 12 }}
        accessibilityLayer={false}
        style={{ width: "100%", height: "100%" }}
      >
        <CartesianGrid stroke="hsl(var(--border))" strokeDasharray="3 5" strokeOpacity={0.42} />
        {horizontal ? (
          <>
            <XAxis type="number" hide />
            <YAxis type="category" dataKey="name" width={64} tick={{ fontSize: 11, fill: "hsl(var(--muted))" }} />
          </>
        ) : (
          <>
            <XAxis dataKey="name" tick={{ fontSize: 11, fill: "hsl(var(--muted))" }} />
            <YAxis hide />
          </>
        )}
        {showTooltip && (
          <Tooltip isAnimationActive={false} formatter={(v) => valueFormatter(Number(v))} contentStyle={tooltipStyle} />
        )}
        <Bar dataKey="value" isAnimationActive={false}>
          {data.map((d, i) => (
            <PieSlice key={i} fill={d.color} />
          ))}
        </Bar>
      </BarChart>
    </div>
  );
}

const tooltipStyle = {
  background: "hsl(var(--panel))",
  border: "1px solid hsl(var(--border))",
  borderRadius: 6,
  fontSize: 11,
  color: "hsl(var(--fg))",
} as const;
