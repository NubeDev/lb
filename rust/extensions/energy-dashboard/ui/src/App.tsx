import { useEffect, useState } from "react";
import { BarChart, Bar, XAxis, YAxis, Tooltip, ResponsiveContainer, CartesianGrid } from "recharts";
import { bridge } from "./bridge";

/*
 * Energy dashboard — real data from the `demo-buildings` sqlite source, reached ONLY through
 * the federated bridge (`bridge.call("federation.query", …)`). The host re-checks
 * `mcp:federation.query:call` on every call under the install grant; the page never sees a
 * token, a DB, or a fetch. Themed by the scoped tokens in tokens.css (the `.lbx-energy-dashboard`
 * root wrapper in mount.tsx anchors them).
 */

type Row = { site: string; kwh: number };

export function App() {
  const [rows, setRows] = useState<Row[] | null>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    const sql =
      "SELECT s.name AS site, ROUND(SUM(pr.value),1) AS kwh " +
      "FROM point_reading pr " +
      "JOIN point p ON p.id = pr.point_id " +
      "JOIN meter m ON m.id = p.meter_id " +
      "JOIN site s ON s.id = m.site_id " +
      "WHERE p.name LIKE '%Energy kWh%' " +
      "GROUP BY s.name ORDER BY kwh DESC LIMIT 8";
    bridge
      .call<{ columns: string[]; rows: unknown[][] }>("federation.query", {
        source: "demo-buildings",
        sql,
      })
      .then((res) => {
        const parsed = (res.rows ?? []).map((r) => ({
          site: String(r[0]),
          kwh: Number(r[1]),
        }));
        setRows(parsed);
      })
      .catch((e: unknown) => setErr(e === null || e === undefined ? String(e) : (e as Error).message ?? String(e)));
  }, []);

  const total = rows ? rows.reduce((acc, r) => acc + r.kwh, 0) : 0;

  return (
    <section style={{ padding: 24, display: "flex", flexDirection: "column", gap: 16 }}>
      <header>
        <h2 style={{ margin: 0, fontSize: 18, fontWeight: 600, color: "hsl(var(--lbx-fg))" }}>
          Energy dashboard
        </h2>
        <p style={{ margin: "4px 0 0", fontSize: 13, color: "hsl(var(--lbx-muted))" }}>
          Total kWh by site — live from <code>demo-buildings</code> via federation.query.
        </p>
      </header>

      {err && (
        <div style={{ color: "hsl(var(--lbx-destructive))", fontSize: 13 }}>
          query failed: {err}
        </div>
      )}

      {rows && (
        <>
          <div
            style={{
              background: "hsl(var(--lbx-card))",
              border: "1px solid hsl(var(--lbx-border))",
              borderRadius: "calc(var(--lbx-radius))",
              padding: 16,
            }}
          >
            <h3 style={{ margin: "0 0 12px", fontSize: 14, fontWeight: 500, color: "hsl(var(--lbx-card-foreground))" }}>
              kWh by site
            </h3>
            <div style={{ height: 280, width: "100%" }}>
              <ResponsiveContainer>
                <BarChart data={rows} margin={{ top: 8, right: 8, bottom: 8, left: 8 }}>
                  <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--lbx-border))" />
                  <XAxis
                    dataKey="site"
                    stroke="hsl(var(--lbx-muted))"
                    tickLine={false}
                    axisLine={false}
                    fontSize={11}
                    angle={-20}
                    textAnchor="end"
                    height={70}
                    interval={0}
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
                    cursor={{ fill: "hsl(var(--lbx-muted) / 0.15)" }}
                  />
                  <Bar dataKey="kwh" fill="hsl(var(--lbx-chart-1))" radius={[4, 4, 0, 0]} />
                </BarChart>
              </ResponsiveContainer>
            </div>
          </div>

          <div
            style={{
              background: "hsl(var(--lbx-card))",
              border: "1px solid hsl(var(--lbx-border))",
              borderRadius: "calc(var(--lbx-radius))",
              padding: 16,
            }}
          >
            <h3 style={{ margin: "0 0 12px", fontSize: 14, fontWeight: 500, color: "hsl(var(--lbx-card-foreground))" }}>
              Sites — total {total.toLocaleString(undefined, { maximumFractionDigits: 1 })} kWh
            </h3>
            <table style={{ width: "100%", borderCollapse: "collapse", fontSize: 13 }}>
              <thead>
                <tr style={{ color: "hsl(var(--lbx-muted))", textAlign: "left" }}>
                  <th style={{ padding: "6px 8px", borderBottom: "1px solid hsl(var(--lbx-border))" }}>Site</th>
                  <th style={{ padding: "6px 8px", borderBottom: "1px solid hsl(var(--lbx-border))", textAlign: "right" }}>kWh</th>
                </tr>
              </thead>
              <tbody>
                {rows.map((r) => (
                  <tr key={r.site} style={{ color: "hsl(var(--lbx-card-foreground))" }}>
                    <td style={{ padding: "6px 8px", borderBottom: "1px solid hsl(var(--lbx-border))" }}>{r.site}</td>
                    <td style={{ padding: "6px 8px", borderBottom: "1px solid hsl(var(--lbx-border))", textAlign: "right" }}>
                      {r.kwh.toLocaleString()}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </>
      )}

      {!rows && !err && (
        <div style={{ color: "hsl(var(--lbx-muted))", fontSize: 13 }}>loading…</div>
      )}
    </section>
  );
}
