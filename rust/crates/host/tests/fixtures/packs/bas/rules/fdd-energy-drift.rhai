// name: FDD · Energy baseline drift
// Each energy/gas meter's last 7 days vs its own prior 3+ weeks — a self-baseline, so
// no absolute thresholds to maintain per site. Ratio ≈ 1.0 is normal seasonal wobble;
// the injected scenario (meter-300, apartments +45%) lands ≈ 1.46. Small baselines
// (summer gas ~0) are skipped rather than divided by.
// (Dialect: DataFusion + sqlite pushdown — see fdd-cooling-failure.rhai.)
let days = query("demo-buildings", `
  SELECT DISTINCT substr(time,1,10) AS d FROM point_reading ORDER BY d DESC LIMIT 7
`).records();
let cutoff = days[days.len() - 1].d;

let rows = query("demo-buildings", `
SELECT m.id AS meter_id, m.name AS meter_name, s.name AS site,
       ROUND(AVG(CASE WHEN pr.time >= '${cutoff}' THEN pr.value END), 3) AS recent_avg,
       ROUND(AVG(CASE WHEN pr.time <  '${cutoff}' THEN pr.value END), 3) AS baseline_avg
FROM point_reading pr
JOIN point p ON p.id = pr.point_id
JOIN meter m ON m.id = p.meter_id
JOIN site  s ON s.id = m.site_id
WHERE p.name IN ('Energy kWh', 'HVAC Energy kWh', 'Gas kWh')
GROUP BY m.id, m.name, s.name
ORDER BY recent_avg DESC
`).records();

for r in rows {
  if r.baseline_avg > 0.5 {
    let ratio = r.recent_avg / r.baseline_avg;
    if ratio > 1.25 {
      insight.raise(#{
        dedup_key: "fdd:energy-drift:" + r.meter_id,
        severity: if ratio > 1.5 { "critical" } else { "warning" },
        title: r.site + " " + r.meter_name + ": consumption up vs baseline",
        body: #{ meter: r.meter_id, site: r.site,
                 recent_avg_kwh: r.recent_avg, baseline_avg_kwh: r.baseline_avg,
                 ratio: ratio, window: "7d vs prior weeks",
                 note: "step change in consumption — new load, failed control, or metering error" },
        tags: #{ area: "fdd", issue: "energy-drift", meter: r.meter_id },
      });
      emit(#{ level: "warning",
              msg: r.site + " " + r.meter_name + " at " + ratio + "x its baseline" });
    }
  }
}

rows
