// name: FDD · Portfolio energy intensity (kWh/m²)
// The portfolio-ranking rule from the Data → insight wizard (dataToInsight.ts), kept in
// the pack so the FDD sweep also produces the cross-site league table: total kWh ÷ floor
// area (the `area` site tag), with a durable insight per over-budget building.
let rows = query("demo-buildings", `
  SELECT s.name AS building,
    ROUND(SUM(pr.value) / CAST(REPLACE(a.val,' m2','') AS DOUBLE), 2) AS kwh_per_m2
  FROM point_reading pr
  JOIN point p ON p.id = pr.point_id
  JOIN meter m ON m.id = p.meter_id
  JOIN site  s ON s.id = m.site_id
  JOIN site_tag a ON a.site_id = s.id AND a.tag = 'area'
  WHERE p.name = 'Energy kWh'
  GROUP BY s.id, s.name, a.val
  ORDER BY kwh_per_m2 DESC
`).records();

for r in rows {
  let building  = r.building;
  let intensity = r.kwh_per_m2;
  if intensity > 1.0 {
    insight.raise(#{
      dedup_key: "energy-intensity-high:" + building,
      severity: if intensity > 2.0 { "critical" } else { "warning" },
      title: building + " energy intensity high",
      body: #{ building: building, kwh_per_m2: intensity, budget: 1.0 },
      tags: #{ area: "energy", building: building },
    });
  }
}

rows
