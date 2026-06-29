// The in-channel chart renderer (channels-query-charts scope). It renders the HOST-COMPUTED
// `ChartSpec` verbatim — the UI never re-derives which chart to draw (the host picked it so every
// subscriber agrees). Switches on `chart.type`: line / bar / histogram. Uses `recharts` (already a
// dependency — no new heavy dep). RENDER ONLY; the rows + spec are passed in (FILE-LAYOUT).

import {
  Bar,
  BarChart,
  CartesianGrid,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";

import type { ChartSpec } from "@/lib/channel/payload.types";

interface Props {
  chart: ChartSpec;
  rows: Record<string, unknown>[];
}

const COLORS = ["#6366f1", "#22c55e", "#f59e0b", "#ec4899", "#06b6d4"];

/** Bucket a single numeric column into `bins` evenly-spaced buckets for a histogram. The host only
 *  suggests the bucket count; the UI does the binning at render time (the rows carry the values). */
function histogram(rows: Record<string, unknown>[], field: string, bins: number) {
  const nums = rows
    .map((r) => Number(r[field]))
    .filter((n) => Number.isFinite(n));
  if (nums.length === 0) return [];
  const min = Math.min(...nums);
  const max = Math.max(...nums);
  const width = (max - min) / bins || 1;
  const buckets = Array.from({ length: bins }, (_, i) => ({
    bucket: `${(min + i * width).toFixed(1)}`,
    count: 0,
  }));
  for (const n of nums) {
    const idx = Math.min(bins - 1, Math.max(0, Math.floor((n - min) / width)));
    buckets[idx].count += 1;
  }
  return buckets;
}

export function ChartView({ chart, rows }: Props) {
  if (chart.type === "histogram") {
    const data = histogram(rows, chart.x, chart.bins ?? 10);
    return (
      <div aria-label="histogram chart" className="h-56 w-full">
        <ResponsiveContainer width="100%" height="100%">
          <BarChart data={data}>
            <CartesianGrid strokeDasharray="3 3" opacity={0.2} />
            <XAxis dataKey="bucket" fontSize={11} />
            <YAxis fontSize={11} allowDecimals={false} />
            <Tooltip />
            <Bar dataKey="count" fill={COLORS[0]} />
          </BarChart>
        </ResponsiveContainer>
      </div>
    );
  }

  if (chart.type === "bar") {
    return (
      <div aria-label="bar chart" className="h-56 w-full">
        <ResponsiveContainer width="100%" height="100%">
          <BarChart data={rows}>
            <CartesianGrid strokeDasharray="3 3" opacity={0.2} />
            <XAxis dataKey={chart.x} fontSize={11} />
            <YAxis fontSize={11} />
            <Tooltip />
            {chart.series.map((s, i) => (
              <Bar key={s.field} dataKey={s.field} fill={COLORS[i % COLORS.length]} />
            ))}
          </BarChart>
        </ResponsiveContainer>
      </div>
    );
  }

  return (
    <div aria-label="line chart" className="h-56 w-full">
      <ResponsiveContainer width="100%" height="100%">
        <LineChart data={rows}>
          <CartesianGrid strokeDasharray="3 3" opacity={0.2} />
          <XAxis dataKey={chart.x} fontSize={11} />
          <YAxis fontSize={11} />
          <Tooltip />
          {chart.series.map((s, i) => (
            <Line
              key={s.field}
              type="monotone"
              dataKey={s.field}
              stroke={COLORS[i % COLORS.length]}
              dot={false}
            />
          ))}
        </LineChart>
      </ResponsiveContainer>
    </div>
  );
}
