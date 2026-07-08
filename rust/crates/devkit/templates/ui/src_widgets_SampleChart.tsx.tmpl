import { LineChart, Line, XAxis, YAxis, Tooltip, ResponsiveContainer } from "recharts";

/*
 * A sample recharts chart colored from the scoped theme tokens (external-agent-authoring scope S3).
 *
 * The series stroke reads `hsl(var(--lbx-chart-1))` — the alias the host shell's `--chart-1` flows
 * through (light AND dark) when federated, and the standalone fallback when developed in isolation.
 * An agent that fills in real data (the energy-dashboard ask) keeps this stroke and gets the
 * workspace's chart palette for free — correct-by-construction theming. No CSS-var-reading hack
 * (recharts can't read a CSS var directly; the alias resolves at the computed-property layer and
 * the inline style reads the resolved value).
 */

const sampleData = [
  { hour: "00", value: 2.1 },
  { hour: "03", value: 1.8 },
  { hour: "06", value: 3.4 },
  { hour: "09", value: 4.7 },
  { hour: "12", value: 5.2 },
  { hour: "15", value: 4.9 },
  { hour: "18", value: 4.1 },
  { hour: "21", value: 2.8 },
];

export function SampleChart() {
  return (
    <div style={{ height: 220, width: "100%" }}>
      <ResponsiveContainer>
        <LineChart data={sampleData} margin={{ top: 8, right: 8, bottom: 0, left: 0 }}>
          <XAxis
            dataKey="hour"
            stroke="hsl(var(--lbx-muted))"
            tickLine={false}
            axisLine={false}
            fontSize={12}
          />
          <YAxis
            stroke="hsl(var(--lbx-muted))"
            tickLine={false}
            axisLine={false}
            fontSize={12}
          />
          <Tooltip
            contentStyle={{
              background: "hsl(var(--lbx-card))",
              border: "1px solid hsl(var(--lbx-border))",
              borderRadius: "calc(var(--lbx-radius))",
              color: "hsl(var(--lbx-fg))",
              fontSize: 12,
            }}
            labelStyle={{ color: "hsl(var(--lbx-muted))" }}
          />
          <Line
            type="monotone"
            dataKey="value"
            stroke="hsl(var(--lbx-chart-1))"
            strokeWidth={2}
            dot={false}
          />
        </LineChart>
      </ResponsiveContainer>
    </div>
  );
}
