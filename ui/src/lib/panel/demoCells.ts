// The demo `Cell` builders bound to the `demo-buildings` federation source (setup scope, shared by the
// Data→insight wizards AND the reports PanelPicker). One place for the prefilled SQL + the two pure
// builders that turn it into a real v3 `Cell`: a timeseries panel and a render-template panel. They
// hold NO markup and issue NO verbs (the wizards / picker own those); they are the cell factory, kept
// out of the feature flows so the flows stay thin (FILE-LAYOUT).
//
// MOVED here from features/admin/setup/dataToInsight.ts so the reports PanelPicker no longer reaches
// across into features/admin/setup/* (the cross-feature import the reports demo-pass shortcut left).
// The wizard-only artifacts (DEMO_RULE / DEMO_DSN / DEMO_ENDPOINT) stay in dataToInsight.ts and
// re-import the builders from here — the wizard imports are unchanged.
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

/** Step 2's preloaded query — hourly average energy per site over the last 4 days. It returns a LONG
 *  frame (one row per hour × site), which the panel in step 3 pivots into one line per site via the
 *  plot spec's `seriesField: "site"` (see {@link DEMO_PLOT}). The SELECT aliases (site / hour /
 *  avg_energy) are the contract those plot fields reference — keep them in sync. */
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

/** The plot spec that draws the query as ONE LINE PER SITE. The query returns a LONG frame — one row
 *  per (hour, site) with an `avg_energy` value — so the timeseries renderer needs a `PlotSpec` with
 *  `seriesField: "site"` to pivot it into wide, per-site series (`readPlotSpec`/`buildPlot`). Without
 *  this the renderer falls into its single-value path and collapses every site into one line. The
 *  column names MUST match the SELECT aliases in {@link DEMO_SQL} (site / hour / avg_energy). */
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

/** Build a real v3 `view:"template"` `Cell` bound to `source`'s federation query for `sql`, rendering
 *  the given template `code`. The sibling of {@link timeseriesCell}: same `federation.query` source
 *  binding (so `TemplateView`'s `usePanelData` feeds the same rows the SQL step ran), but the template
 *  view + `options.code` — the key `TemplateView` reads (`options.code ?? options.templateId`). The
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
