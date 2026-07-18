// name: FDD · After-hours operation
// Compares each unit's fan-run slots against its own scheduled window (the
// operatingStart/operatingEnd meter tags — the Haystack sched hint the seeder writes)
// and totals run-hours outside it. The injected scenario (meter-211, mall AHU-2 on a
// 10:00-21:00 schedule) runs 24/7 → ~13 h/day after hours; healthy units are ~0.
// Slots are 15 min (× 0.25 h); window: last 7 days. Hours come from substr on the ISO
// timestamp; sched tags are 'HH:00' so an hour-level compare is exact ('24:00' → 24,
// never matched — 0-24 units can have no after-hours by definition).
// (Dialect: DataFusion + sqlite pushdown — see fdd-cooling-failure.rhai.)
let days = query("demo-buildings", `
  SELECT DISTINCT substr(time,1,10) AS d FROM point_reading ORDER BY d DESC LIMIT 7
`).records();
let cutoff = days[days.len() - 1].d;

let rows = query("demo-buildings", `
SELECT m.id AS equip, m.name AS equip_name, s.name AS site,
       os.val AS op_start, oe.val AS op_end,
       ROUND(SUM(CASE WHEN CAST(substr(pr.time,12,2) AS INT) <  CAST(substr(os.val,1,2) AS INT)
                        OR CAST(substr(pr.time,12,2) AS INT) >= CAST(substr(oe.val,1,2) AS INT)
                      THEN pr.value ELSE 0 END) * 0.25 / 7.0, 1) AS after_hours_per_day,
       ROUND(SUM(pr.value) * 0.25 / 7.0, 1) AS run_hours_per_day
FROM point_reading pr
JOIN point p       ON p.id = pr.point_id
JOIN meter m       ON m.id = p.meter_id
JOIN meter_tag os  ON os.meter_id = m.id AND os.tag = 'operatingStart'
JOIN meter_tag oe  ON oe.meter_id = m.id AND oe.tag = 'operatingEnd'
JOIN site s        ON s.id = m.site_id
WHERE p.id LIKE '%-fan' AND pr.time >= '${cutoff}'
GROUP BY m.id, m.name, s.name, os.val, oe.val
ORDER BY after_hours_per_day DESC
`).records();

for r in rows {
  if r.after_hours_per_day >= 2.0 {
    insight.raise(#{
      dedup_key: "fdd:after-hours:" + r.equip,
      severity: if r.after_hours_per_day >= 6.0 { "critical" } else { "warning" },
      title: r.site + " " + r.equip_name + ": running outside its schedule",
      body: #{ equip: r.equip, site: r.site,
               schedule: r.op_start + "-" + r.op_end,
               after_hours_per_day: r.after_hours_per_day,
               run_hours_per_day: r.run_hours_per_day,
               note: "pure energy waste — check the time schedule / BMS override" },
      tags: #{ area: "fdd", issue: "after-hours", equip: r.equip },
    });
    emit(#{ level: "warning",
            msg: r.equip_name + " @ " + r.site + " runs " + r.after_hours_per_day + " h/day after hours" });
  }
}

rows
