// name: FDD · Zone-temp sensor flatline
// A live zone-temp sensor always moves (occupancy, HVAC, weather). If the last day's
// spread (max-min) is ~0 over enough samples, the sensor is frozen, unwired, or the
// trend is dead — and every control loop hanging off it is flying blind. Healthy
// zone temps in the demo spread 0.6+ °C/day; the injected scenario (meter-020-zt) is 0.
// (Dialect: DataFusion + sqlite pushdown — see fdd-cooling-failure.rhai.)
let days = query("demo-buildings", `
  SELECT DISTINCT substr(time,1,10) AS d FROM point_reading ORDER BY d DESC LIMIT 1
`).records();
let cutoff = days[days.len() - 1].d;

let rows = query("demo-buildings", `
SELECT p.id AS point_id, p.name AS point_name, m.id AS equip, m.name AS equip_name, s.name AS site,
       ROUND(MAX(pr.value) - MIN(pr.value), 3) AS spread_degc,
       ROUND(AVG(pr.value), 2) AS stuck_at,
       COUNT(*) AS samples
FROM point_reading pr
JOIN point p ON p.id = pr.point_id
JOIN meter m ON m.id = p.meter_id
JOIN site  s ON s.id = m.site_id
WHERE p.id LIKE '%-zt' AND pr.time >= '${cutoff}'
GROUP BY p.id, p.name, m.id, m.name, s.name
ORDER BY spread_degc ASC
`).records();

for r in rows {
  if r.samples >= 24 && r.spread_degc < 0.1 {
    insight.raise(#{
      dedup_key: "fdd:sensor-flatline:" + r.point_id,
      severity: "critical",
      title: r.site + " " + r.equip_name + ": zone-temp sensor flatlined",
      body: #{ point: r.point_id, equip: r.equip, site: r.site,
               stuck_at_degc: r.stuck_at, spread_24h_degc: r.spread_degc,
               samples: r.samples,
               note: "control loops using this sensor are flying blind" },
      // The data that proves it (insight-evidence-scope.md). A SINGLE-POINT rule is the easy
      // case: the series it judged is the series you plot, so `series` and `query` describe the
      // same underlying readings and the flatline is visible on the chart.
      // No `threshold` — the rule judges a SPREAD (max-min < 0.1 degC), which is not a value on
      // the temperature series. Drawing 0.1 degC across a zone-temp chart would be nonsense; the
      // field is "the threshold crossed, in the series' own units" and this one isn't.
      evidence: #{
        source: "demo-buildings",
        series: [#{ sql: "SELECT time, value FROM point_reading WHERE point_id = '" + r.point_id + "' ORDER BY time",
                    label: r.point_name, unit: "degC" }],
      },
      tags: #{ area: "fdd", issue: "sensor-flatline", equip: r.equip },
    });
    emit(#{ level: "critical",
            msg: r.point_name + " on " + r.equip_name + " @ " + r.site + " frozen at " + r.stuck_at + " degC" });
  }
}

rows
