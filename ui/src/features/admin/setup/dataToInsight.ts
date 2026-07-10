// The canonical artifacts the Data → insight wizard walks a user through (setup scope). One place for
// the three prefilled strings the wizard preloads — the SQL query, the timeseries panel it builds, and
// the Rhai rule — plus the pure helper that turns the SQL into a real v3 timeseries `Cell` bound to the
// chosen federation datasource. It holds NO markup and issues NO verbs (the wizard owns those); it is
// the wizard's data + one cell-builder, kept out of the flow file so the flow stays thin (FILE-LAYOUT).
//
// Why these exact strings: they target the shipped `demo-buildings` SQLite datasource (the same one the
// Query workbench + the host's `buildings_examples.json` regression rule use — `point_reading` / `site`
// / `meter` / `point_tag`). Reusing that real demo domain means every step runs against real data, not
// a fixture: the query returns rows, the panel renders, the rule raises a real insight.

import type { Cell } from "@/lib/dashboard";
import type { PlotSpec } from "@/lib/charts";
import { defaultCell } from "@/lib/panel-kit/defaultCell";
import { defaultOptionsForView } from "@/features/panel-builder/viewOptions";

/** The default federation datasource the wizard preloads (the shipped buildings demo). The user can
 *  pick any registered source in step 1; this is only the suggested/registered default. */
export const DEFAULT_SOURCE = "demo-buildings";

/** The suggested endpoint + DSN for registering `demo-buildings` if the workspace has no source yet.
 *  Mirrors the Query-workbench test's `registerDemoSource` (the real `datasource.add` roster path). */
export const DEMO_ENDPOINT = "127.0.0.1:0";
export const DEMO_DSN = "/tmp/lb-demo-buildings.db";

/** Step 2's preloaded query — hourly average energy per site over the last 4 days. It returns a LONG
 *  frame (one row per hour × site), which the panel in step 3 pivots into one line per site via the
 *  plot spec's `seriesField: "site"` (see `DEMO_PLOT`). The SELECT aliases (site / hour / avg_energy)
 *  are the contract those plot fields reference — keep them in sync. */
export const DEMO_SQL = `SELECT
  s.name                              AS site,
  substr(r.time, 1, 13) || ':00:00'   AS hour,
  AVG(r.value)                        AS avg_energy
FROM point_reading r
JOIN point     p ON r.point_id = p.id
JOIN meter     m ON p.meter_id = m.id
JOIN site      s ON m.site_id  = s.id
JOIN point_tag u ON u.point_id = p.id AND u.tag = 'unit' AND u.val = 'kWh'
WHERE CAST(r.time AS TIMESTAMP) >= now() - INTERVAL '4 days'
GROUP BY hour, s.name
ORDER BY s.name, hour;`;

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

/** The plot spec that draws the query as ONE LINE PER SITE. The query returns a LONG frame — one row
 *  per (hour, site) with an `avg_energy` value — so the timeseries renderer needs a `PlotSpec` with
 *  `seriesField: "site"` to pivot it into wide, per-site series (`readPlotSpec`/`buildPlot`). Without
 *  this the renderer falls into its single-value path and collapses every site into one line. The
 *  column names MUST match the SELECT aliases in `DEMO_SQL` (site / hour / avg_energy). */
export const DEMO_PLOT: PlotSpec = {
  type: "line",
  xField: "hour",
  yFields: ["avg_energy"],
  seriesField: "site",
  smooth: true,
};

/** Build a real v3 timeseries `Cell` bound to `source`'s federation query for `sql`. Mirrors the panel
 *  wizard's `seedFromPrefill` (a `federation.query` target + the SQL) but seeds the TIMESERIES view AND
 *  a `plot` spec that splits by site — the wizard saves this exact cell into a fresh dashboard, so what
 *  step 3 previews is what the dashboard shows. The split is the `plot.seriesField`, applied to the
 *  query's long (site, hour, value) frame. */
export function timeseriesCell(ws: string, source: string, sql: string, title: string): Cell {
  const base = defaultCell("timeseries", `panel-${title}`, undefined, defaultOptionsForView("timeseries"));
  return {
    ...base,
    view: "timeseries",
    title,
    sources: [
      {
        refId: "A",
        tool: "federation.query",
        args: { source, sql },
        datasource: { type: "federation", uid: `datasource:${ws}:${source}` },
      },
    ],
    options: {
      ...base.options,
      // The multi-series plot (one line per site) + the SQL builder state (so the cell edits cleanly
      // later, matching seedFromPrefill).
      plot: DEMO_PLOT,
      sql: { mode: "code", rawSql: sql, format: "table" },
    },
  };
}
// The render-template wizard's starter widgets live in `templateGallery.ts` (three polished examples,
// each with its own summary SQL) — not here. `dataToInsight.ts` keeps the data→insight wizard's
// artifacts (DEMO_SQL / DEMO_RULE / DEMO_PLOT) + the cell builders both wizards share.

/** Build a real v3 `view:"template"` `Cell` bound to `source`'s federation query for `sql`, rendering
 *  the given template `code`. The sibling of `timeseriesCell`: same `federation.query` source binding
 *  (so `TemplateView`'s `usePanelData` feeds the same rows the SQL step ran), but the template view +
 *  `options.code` — the key `TemplateView` reads (`options.code ?? options.templateId`). The
 *  render-template wizard previews this exact cell live and saves the same `code` as a durable
 *  `render_template`, so what the user designs is what persists (no drift). */
export function templateCell(ws: string, source: string, sql: string, code: string, title: string): Cell {
  const base = defaultCell("template", `panel-${title}`, undefined, defaultOptionsForView("template"));
  return {
    ...base,
    view: "template",
    title,
    sources: [
      {
        refId: "A",
        tool: "federation.query",
        args: { source, sql },
        datasource: { type: "federation", uid: `datasource:${ws}:${source}` },
      },
    ],
    options: { ...base.options, code, sql: { mode: "code", rawSql: sql, format: "table" } },
  };
}
