// The canonical artifacts the Data → insight wizard walks a user through (setup scope). After the
// shared cell builders + their inputs were lifted to `@/lib/panel` (so the reports PanelPicker no
// longer reaches across into this feature), this file holds ONLY the wizard-specific artifacts: the
// Rhai rule, and the suggested datasource endpoint/DSN for registering `demo-buildings`. The shared
// builders (`timeseriesCell`/`templateCell`), the demo SQL, and the starter gallery are re-exported
// from `@/lib/panel` so the wizard's existing imports stay green.
//
// Why these exact strings: they target the shipped `demo-buildings` SQLite datasource (the same one the
// Query workbench + the host's `buildings_examples.json` regression rule use — `point_reading` / `site`
// / `meter` / `point_tag`). Reusing that real demo domain means every step runs against real data, not
// a fixture: the query returns rows, the panel renders, the rule raises a real insight.

// Re-export the shared demo cell builders + inputs (now in lib/panel) so wizard imports are unchanged.
export {
  DEFAULT_SOURCE,
  DEMO_SQL,
  DEMO_PLOT,
  timeseriesCell,
  templateCell,
} from "@/lib/panel";
// The render-template wizard's starter widgets live in `@/lib/panel/demoGallery` (re-exported by
// `@/lib/panel`); import them directly from there.

/** The suggested endpoint + DSN for registering `demo-buildings` if the workspace has no source yet.
 *  Mirrors the Query-workbench test's `registerDemoSource` (the real `datasource.add` roster path).
 *  Wizard-only — not used by the reports PanelPicker. */
export const DEMO_ENDPOINT = "127.0.0.1:0";
export const DEMO_DSN = "/tmp/lb-demo-buildings.db";

/** Step 5's preloaded rule (Rhai) — ranks every building by energy intensity and raises a durable,
 *  deduped insight on anything over budget. This is the rule the user PREVIEWS and RUNs; it is not
 *  edited here (the Rules workbench owns authoring). Running it is what populates step 6's insights. */
export const DEMO_RULE = `// Rank every building by energy intensity (total kWh ÷ floor area from the \`area\` site tag).
// SQL rides in a \`backtick\` raw string so it can span lines (a "double-quoted" string can't).
let rows = query("demo-buildings", \`
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
\`).records();

// Each row is a map keyed by the SELECT aliases: r.building, r.kwh_per_m2.
// Raise a durable insight on anything over budget — deduped per building, so a re-run bumps its
// \`count\` instead of duplicating.
for r in rows {
  let building  = r.building;
  let intensity = r.kwh_per_m2;
  let key = "energy-intensity-high:" + building;
  if intensity > 1.0 {
    insight.raise(#{
      dedup_key: key,
      severity: if intensity > 2.0 { "critical" } else { "warning" },
      title: building + " energy intensity high",
      body: #{ building: building, kwh_per_m2: intensity, budget: 1.0 },
      tags: #{ area: "energy", building: building },
    });
  }
}

rows   // the ranked table is still the result (renders as a panel / reads back to the caller)`;
