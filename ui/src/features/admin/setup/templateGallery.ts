// The render-template wizard's STARTER GALLERY (setup scope) — three worked, good-looking template
// widgets the Design step offers as starting points. The whole point of the template widget is building
// a view we DON'T already have pre-made, so these lean into that: a spotlight leaderboard, a KPI stat
// row, and a bar-meter ranking — none of which is a stock chart type.
//
// Each example ships its OWN summary SQL (a small, per-site aggregate — not 840 raw hourly rows) so the
// widget renders a handful of clean rows with pre-computed, pre-ROUNDED columns. That matters because
// the template engine is pure `{{…}}` interpolation with NO math/formatting helpers: anything the markup
// needs (a rounded number, a 0–100 bar width, a rank) must already be a column. So the SQL does the
// arithmetic and the template does the layout.
//
// Styling rules the markup obeys (the sanitizer + in-process shell constraints):
//   - SVG is NOT allowed (DOMPurify strips <svg>) — visuals are CSS only (gradients, conic/radial
//     backgrounds, borders, bar fills) + unicode/emoji glyphs. Inline `<style>` blocks ARE allowed.
//   - Inline styles + host theme tokens (hsl(var(--fg|muted|accent|panel|border|bg))) so both themes
//     work and the widget looks native. Numbers use font-variant-numeric:tabular-nums.
//   - The root fills the tile (height:100%) and inner scroll regions use overflow:auto.
//
// One responsibility per file (FILE-LAYOUT): the gallery data (id, label, description, sql, code). No
// markup rendering, no verbs — the wizard's Design step consumes this list.

export interface TemplateExample {
  id: string;
  /** Card label in the gallery picker. */
  label: string;
  /** One-line description of what the widget shows. */
  description: string;
  /** The summary query this example renders (per-site aggregate, pre-rounded, with any bar/rank column). */
  sql: string;
  /** The template HTML (eval-free `{{…}}` engine). */
  code: string;
}

// ── Shared SQL: a compact per-site summary over the buildings demo. One row per site with total +
//    peak kWh, the share of the busiest site (0–100, for bar widths), and a dense rank. Everything is
//    ROUNDed in SQL because the template can't format. `pct` is the bar width; `rnk` is the position. ──
const SUMMARY_SQL = `WITH per_site AS (
  SELECT
    s.name                       AS site,
    ROUND(SUM(r.value), 1)       AS total_kwh,
    ROUND(MAX(r.value), 2)       AS peak_kwh,
    ROUND(AVG(r.value), 2)       AS avg_kwh
  FROM point_reading r
  JOIN point     p ON r.point_id = p.id
  JOIN meter     m ON p.meter_id = m.id
  JOIN site      s ON m.site_id  = s.id
  JOIN point_tag u ON u.point_id = p.id AND u.tag = 'unit' AND u.val = 'kWh'
  WHERE CAST(r.time AS TIMESTAMP) >= now() - INTERVAL '4 days'
  GROUP BY s.name
)
SELECT
  site,
  total_kwh,
  peak_kwh,
  avg_kwh,
  ROUND(100.0 * total_kwh / MAX(total_kwh) OVER (), 0) AS pct,
  RANK() OVER (ORDER BY total_kwh DESC)                AS rnk
FROM per_site
ORDER BY total_kwh DESC;`;

// ── Example 1: the "top consumer" spotlight leaderboard. A hero card for the #1 site (big number, a
//    conic-gradient ring drawn in CSS, its share), then a ranked list with inline bar fills. ──
const LEADER_CODE = `<style>
  .tw-root{display:flex;flex-direction:column;gap:14px;height:100%;padding:4px;color:hsl(var(--fg));font-size:13px}
  .tw-hero{display:flex;align-items:center;gap:16px;padding:16px 18px;border-radius:16px;
    background:linear-gradient(135deg,hsl(var(--accent)/0.16),hsl(var(--accent)/0.04));
    border:1px solid hsl(var(--accent)/0.25)}
  .tw-ring{width:64px;height:64px;border-radius:50%;flex:none;display:grid;place-items:center;
    background:conic-gradient(hsl(var(--accent)) calc(var(--p,60)*1%),hsl(var(--border)) 0)}
  .tw-ring span{width:50px;height:50px;border-radius:50%;background:hsl(var(--panel));display:grid;
    place-items:center;font-size:20px}
  .tw-hero .big{font-size:30px;font-weight:800;line-height:1;font-variant-numeric:tabular-nums;
    color:hsl(var(--accent))}
  .tw-list{display:flex;flex-direction:column;gap:8px;overflow-y:auto}
  .tw-row{position:relative;display:flex;align-items:center;gap:12px;padding:9px 12px;border-radius:12px;
    border:1px solid hsl(var(--border));background:hsl(var(--panel));overflow:hidden}
  .tw-row .fill{position:absolute;inset:0;width:calc(var(--p,0)*1%);
    background:linear-gradient(90deg,hsl(var(--accent)/0.18),hsl(var(--accent)/0.02));z-index:0}
  .tw-row > *{position:relative;z-index:1}
  .tw-rank{width:26px;height:26px;flex:none;border-radius:8px;display:grid;place-items:center;
    font-weight:700;background:hsl(var(--accent)/0.14);color:hsl(var(--accent))}
  .tw-name{flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;font-weight:600}
  .tw-val{font-variant-numeric:tabular-nums;font-weight:700}
  .tw-unit{font-size:10px;color:hsl(var(--muted));margin-left:2px}
  .tw-cap{font-size:10px;letter-spacing:.08em;text-transform:uppercase;color:hsl(var(--muted))}
</style>
<div class="tw-root">
  <div class="tw-hero">
    <div class="tw-ring" style="--p:{{rows.0.pct}}"><span>⚡</span></div>
    <div style="min-width:0">
      <div class="tw-cap">Top energy consumer · last 4 days</div>
      <div style="display:flex;align-items:baseline;gap:8px;margin-top:2px">
        <span class="big">{{rows.0.total_kwh}}</span><span class="tw-unit" style="font-size:13px">kWh</span>
      </div>
      <div style="margin-top:3px;font-weight:600;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">{{rows.0.site}}</div>
    </div>
  </div>
  <div class="tw-cap" style="padding:0 4px">Full ranking</div>
  <div class="tw-list">
    {{#each rows}}<div class="tw-row" style="--p:{{pct}}">
      <div class="fill"></div>
      <div class="tw-rank">{{rnk}}</div>
      <div class="tw-name">{{site}}</div>
      <div class="tw-val">{{total_kwh}}<span class="tw-unit">kWh</span></div>
    </div>{{/each}}
  </div>
</div>`;

// ── Example 2: the KPI stat row. Big, gradient stat tiles — the kind of "at a glance" header a chart
//    can't give you. Uses the last row's fields + rows.length; pure layout. ──
const STATS_CODE = `<style>
  .sw-root{display:flex;flex-direction:column;gap:12px;height:100%;padding:4px;color:hsl(var(--fg))}
  .sw-head{display:flex;align-items:baseline;justify-content:space-between}
  .sw-title{font-size:15px;font-weight:700}
  .sw-sub{font-size:11px;color:hsl(var(--muted))}
  .sw-grid{display:grid;grid-template-columns:repeat(2,1fr);gap:12px;flex:1;min-height:0}
  .sw-tile{position:relative;overflow:hidden;display:flex;flex-direction:column;justify-content:space-between;
    gap:8px;padding:16px;border-radius:16px;border:1px solid hsl(var(--border));background:hsl(var(--panel))}
  .sw-tile::after{content:"";position:absolute;right:-30px;top:-30px;width:110px;height:110px;border-radius:50%;
    background:radial-gradient(circle,hsl(var(--accent)/0.18),transparent 70%)}
  .sw-ico{width:34px;height:34px;border-radius:10px;display:grid;place-items:center;font-size:18px;
    background:hsl(var(--accent)/0.14)}
  .sw-num{font-size:30px;font-weight:800;line-height:1;font-variant-numeric:tabular-nums}
  .sw-lab{font-size:11px;letter-spacing:.06em;text-transform:uppercase;color:hsl(var(--muted))}
  .sw-unit{font-size:13px;font-weight:600;color:hsl(var(--muted));margin-left:3px}
</style>
<div class="sw-root">
  <div class="sw-head">
    <span class="sw-title">Energy overview</span>
    <span class="sw-sub">{{rows.length}} sites · last 4 days</span>
  </div>
  <div class="sw-grid">
    <div class="sw-tile">
      <div class="sw-ico">🏆</div>
      <div><div class="sw-num">{{rows.0.total_kwh}}<span class="sw-unit">kWh</span></div>
      <div class="sw-lab">Top site total</div></div>
    </div>
    <div class="sw-tile">
      <div class="sw-ico">📈</div>
      <div><div class="sw-num">{{rows.0.peak_kwh}}<span class="sw-unit">kWh</span></div>
      <div class="sw-lab">Peak reading</div></div>
    </div>
    <div class="sw-tile">
      <div class="sw-ico">🏢</div>
      <div><div class="sw-num">{{rows.length}}</div>
      <div class="sw-lab">Sites reporting</div></div>
    </div>
    <div class="sw-tile">
      <div class="sw-ico">⚡</div>
      <div><div class="sw-num">{{rows.0.avg_kwh}}<span class="sw-unit">kWh</span></div>
      <div class="sw-lab">Top site avg</div></div>
    </div>
  </div>
</div>`;

// ── Example 3: the bar-meter ranking. A dense, scannable list where each site's bar width IS its share
//    of the leader (the `pct` column). Clean, big, and impossible to get from a stock bar chart tile
//    with this exact styling. ──
const RANKING_CODE = `<style>
  .bw-root{display:flex;flex-direction:column;gap:10px;height:100%;padding:4px;color:hsl(var(--fg))}
  .bw-title{font-size:14px;font-weight:700;display:flex;align-items:center;gap:8px}
  .bw-title .dot{width:9px;height:9px;border-radius:50%;background:hsl(var(--accent))}
  .bw-list{display:flex;flex-direction:column;gap:11px;overflow-y:auto;padding-right:2px}
  .bw-item{display:flex;flex-direction:column;gap:5px}
  .bw-top{display:flex;align-items:baseline;gap:8px}
  .bw-name{flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;font-weight:600;font-size:13px}
  .bw-val{font-variant-numeric:tabular-nums;font-weight:700;font-size:13px}
  .bw-unit{font-size:10px;color:hsl(var(--muted));margin-left:2px}
  .bw-track{height:10px;border-radius:999px;background:hsl(var(--border)/0.6);overflow:hidden}
  .bw-fill{height:100%;border-radius:999px;width:calc(var(--p,0)*1%);
    background:linear-gradient(90deg,hsl(var(--accent)/0.65),hsl(var(--accent)))}
</style>
<div class="bw-root">
  <div class="bw-title"><span class="dot"></span> Energy by site — share of leader</div>
  <div class="bw-list">
    {{#each rows}}<div class="bw-item">
      <div class="bw-top">
        <span class="bw-name">{{rnk}}. {{site}}</span>
        <span class="bw-val">{{total_kwh}}<span class="bw-unit">kWh</span></span>
        <span class="bw-unit" style="font-variant-numeric:tabular-nums">{{pct}}%</span>
      </div>
      <div class="bw-track"><div class="bw-fill" style="--p:{{pct}}"></div></div>
    </div>{{/each}}
  </div>
</div>`;

/** The three starter widgets the Design step offers. The FIRST is the default seed. */
export const TEMPLATE_GALLERY: TemplateExample[] = [
  {
    id: "leader",
    label: "Top consumer spotlight",
    description: "A hero card for the #1 site plus a ranked list with inline energy bars.",
    sql: SUMMARY_SQL,
    code: LEADER_CODE,
  },
  {
    id: "stats",
    label: "Energy stat tiles",
    description: "Big at-a-glance KPI tiles — top total, peak, sites reporting, average.",
    sql: SUMMARY_SQL,
    code: STATS_CODE,
  },
  {
    id: "ranking",
    label: "Bar-meter ranking",
    description: "Every site as a labelled progress bar sized by its share of the leader.",
    sql: SUMMARY_SQL,
    code: RANKING_CODE,
  },
];

/** The default starter (the leaderboard) — the seed the Design step opens with. */
export const DEFAULT_TEMPLATE = TEMPLATE_GALLERY[0]!;
