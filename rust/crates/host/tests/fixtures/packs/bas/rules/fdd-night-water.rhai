// name: FDD · Night-time water flow (leak suspect)
// Buildings barely use water at 00:00-05:00; a leak never sleeps. Compare each water
// meter's overnight average flow to its own daytime average over the last 7 days —
// a self-baselined ratio, so big and small buildings use one threshold. Healthy demo
// meters sit ≤ 0.45; the injected scenario (meter-102, hospital) sits ≈ 0.75.
// (Dialect: DataFusion + sqlite pushdown — see fdd-cooling-failure.rhai.)
let days = query("demo-buildings", `
  SELECT DISTINCT substr(time,1,10) AS d FROM point_reading ORDER BY d DESC LIMIT 7
`).records();
let cutoff = days[days.len() - 1].d;

let rows = query("demo-buildings", `
SELECT m.id AS meter_id, m.name AS meter_name, s.name AS site,
       ROUND(AVG(CASE WHEN CAST(substr(pr.time,12,2) AS INT) < 5
                      THEN pr.value END), 2) AS night_flow,
       ROUND(AVG(CASE WHEN CAST(substr(pr.time,12,2) AS INT) >= 9
                       AND CAST(substr(pr.time,12,2) AS INT) < 17
                      THEN pr.value END), 2) AS day_flow
FROM point_reading pr
JOIN point p ON p.id = pr.point_id
JOIN meter m ON m.id = p.meter_id
JOIN site  s ON s.id = m.site_id
WHERE p.id LIKE '%-flow' AND pr.time >= '${cutoff}'
GROUP BY m.id, m.name, s.name
ORDER BY night_flow DESC
`).records();

for r in rows {
  if r.day_flow > 0.5 {
    let ratio = r.night_flow / r.day_flow;
    if ratio > 0.6 && r.night_flow > 1.0 {
      insight.raise(#{
        dedup_key: "fdd:night-water:" + r.meter_id,
        severity: if ratio > 0.8 { "critical" } else { "warning" },
        title: r.site + ": water flowing all night — possible leak",
        body: #{ meter: r.meter_id, site: r.site,
                 night_flow_lpm: r.night_flow, day_flow_lpm: r.day_flow,
                 night_day_ratio: ratio, window: "7d",
                 note: "overnight flow should be a trickle; sustained flow = leak or stuck valve" },
        tags: #{ area: "fdd", issue: "night-leak", meter: r.meter_id },
      });
      emit(#{ level: "warning",
              msg: r.site + " night flow " + r.night_flow + " L/min vs day " + r.day_flow + " L/min" });
    }
  }
}

rows
