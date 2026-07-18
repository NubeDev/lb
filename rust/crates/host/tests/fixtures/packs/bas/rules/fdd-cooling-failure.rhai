// name: FDD · Cooling failure (compressor runs, no SAT-RAT delta)
// A DX unit that is cooling pulls supply air well below return air. If the compressor
// reports ON but supply tracks return, the compressor is dead / charge is lost / the
// status point lies. Healthy units in the demo sit at dT ≈ -6..-10 °C; the injected
// fault (meter-050) sits at ≈ +1.2. Window: last 3 days, ≥8 compressor-on slots.
//
// SQL dialect note (all fdd-* rules): federation plans with DataFusion and pushes SQL
// down into the sqlite engine, so only functions BOTH dialects share are safe — string
// ops on the ISO 'YYYY-MM-DDT...' timestamps (substr/CAST), never datetime()/to_timestamp().
// The day cutoff is fetched first and interpolated, because scalar subqueries break the
// pushdown translator.
let days = query("demo-buildings", `
  SELECT DISTINCT substr(time,1,10) AS d FROM point_reading ORDER BY d DESC LIMIT 3
`).records();
let cutoff = days[days.len() - 1].d;

let rows = query("demo-buildings", `
SELECT m.id AS equip, m.name AS equip_name, s.name AS site,
       ROUND(AVG(sat.value - rat.value), 2) AS cool_delta,
       COUNT(*) AS comp_slots
FROM point_reading c
JOIN point pc  ON pc.id = c.point_id
JOIN meter m   ON m.id = pc.meter_id
JOIN site  s   ON s.id = m.site_id
JOIN point_reading sat ON sat.point_id = m.id || '-sat' AND sat.time = c.time
JOIN point_reading rat ON rat.point_id = m.id || '-rat' AND rat.time = c.time
WHERE pc.id LIKE '%-cmp' AND c.value >= 0.5 AND c.time >= '${cutoff}'
GROUP BY m.id, m.name, s.name
ORDER BY cool_delta DESC
`).records();

for r in rows {
  if r.comp_slots >= 8 && r.cool_delta > -3.0 {
    insight.raise(#{
      dedup_key: "fdd:cooling-failure:" + r.equip,
      severity: if r.cool_delta > -1.0 { "critical" } else { "warning" },
      title: r.site + " " + r.equip_name + ": compressor runs but does not cool",
      body: #{ equip: r.equip, site: r.site, avg_sat_minus_rat_degc: r.cool_delta,
               comp_on_slots_3d: r.comp_slots,
               expected: "supply at least 3 degC below return while compressor is on" },
      tags: #{ area: "fdd", issue: "cooling-failure", equip: r.equip },
    });
    emit(#{ level: "critical",
            msg: r.equip_name + " @ " + r.site + ": no cooling while compressor on (dT " + r.cool_delta + " degC)" });
  }
}

rows
