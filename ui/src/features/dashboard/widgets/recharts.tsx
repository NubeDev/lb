// Shared Recharts renderers for built-in dashboard widgets. Data access stays in the v1/v2 containers;
// this file only owns the chart SVGs so the built-ins do not drift between hand-drawn implementations.

import {
  Area,
  AreaChart,
  CartesianGrid,
  PolarAngleAxis,
  RadialBar,
  RadialBarChart,
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

export function GaugeArcChart({ fraction, ariaLabel }: { fraction: number; ariaLabel: string }) {
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
          fill="hsl(var(--accent))"
          isAnimationActive={false}
        />
      </RadialBarChart>
    </div>
  );
}
