// name: FDD · Compressor short-cycling
// Counts compressor state flips over the last 3 days (LAG over the status series) and
// converts them to cycles/day. Hysteresis-driven units cycle a handful of times a day;
// the injected scenario (meter-061) flips every occupied slot (~20 cycles/day). Short
// cycling murders compressors — this is the classic maintenance catch.
// (Dialect: DataFusion + sqlite pushdown — see fdd-cooling-failure.rhai.)
let days = query("demo-buildings", `
  SELECT DISTINCT substr(time,1,10) AS d FROM point_reading ORDER BY d DESC LIMIT 3
`).records();
let cutoff = days[days.len() - 1].d;

let rows = query("demo-buildings", `
SELECT c.equip, m.name AS equip_name, s.name AS site,
       ROUND(SUM(CASE WHEN c.prev IS NOT NULL AND c.value <> c.prev THEN 1 ELSE 0 END) / 2.0 / 3.0, 1)
         AS cycles_per_day
FROM (
  SELECT p.meter_id AS equip, pr.point_id, pr.time, pr.value,
         LAG(pr.value) OVER (PARTITION BY pr.point_id ORDER BY pr.time) AS prev
  FROM point_reading pr
  JOIN point p ON p.id = pr.point_id
  WHERE p.id LIKE '%-cmp' AND pr.time >= '${cutoff}'
) c
JOIN meter m ON m.id = c.equip
JOIN site  s ON s.id = m.site_id
GROUP BY c.equip, m.name, s.name
ORDER BY cycles_per_day DESC
`).records();

for r in rows {
  if r.cycles_per_day >= 12.0 {
    insight.raise(#{
      dedup_key: "fdd:short-cycling:" + r.equip,
      severity: if r.cycles_per_day >= 18.0 { "critical" } else { "warning" },
      title: r.site + " " + r.equip_name + ": compressor short-cycling",
      body: #{ equip: r.equip, site: r.site, cycles_per_day: r.cycles_per_day,
               threshold: 12.0, window: "3d",
               note: "rapid on/off wears the compressor; check staging + hysteresis" },
      tags: #{ area: "fdd", issue: "short-cycling", equip: r.equip },
    });
    emit(#{ level: "warning",
            msg: r.equip_name + " @ " + r.site + " cycling " + r.cycles_per_day + "x/day" });
  }
}

rows
