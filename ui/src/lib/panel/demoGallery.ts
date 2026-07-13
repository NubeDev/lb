// The render-template STARTER GALLERY — three worked, good-looking template widgets offered as
// starting points (setup scope's render-template wizard, and now the reports PanelPicker's starter
// widgets). The whole point of the template widget is building a view we DON'T already have pre-made,
// so these lean into that: a spotlight leaderboard, a KPI stat row, a bar-meter ranking, and a
// draft-with-AI scaffold — none of which is a stock chart type.
//
// Each example ships its OWN summary SQL (a small, per-site aggregate — not 840 raw hourly rows) so the
// widget renders a handful of clean rows with pre-computed, pre-ROUNDED columns. That matters because
// the template engine is pure `{{…}}` interpolation with NO math/formatting helpers: anything the markup
// needs (a rounded number, a 0–100 bar width, a rank) must already be a column. So the SQL does the
// arithmetic and the template does the layout.
//
// Styling rules the markup obeys (the sanitizer + in-process shell constraints):
//   - INLINE `style=""` attributes ONLY. The sanitizer STRIPS `<style>` blocks (their contents are
//     dropped) AND CSS custom properties are useless without a stylesheet — so every element carries its
//     own inline style, and a bar width is a literal `width:{{pct}}%`, never a `--p` var.
//   - SVG is NOT allowed (DOMPurify strips <svg>) — visuals are CSS only (gradients, radial backgrounds,
//     borders, box-shadow, bar fills) + unicode/emoji glyphs.
//   - Host theme tokens (hsl(var(--fg|muted|accent|panel|border|bg))) so both themes work and the
//     widget looks native. Numbers use font-variant-numeric:tabular-nums.
//   - The root fills the tile (height:100%;box-sizing:border-box) and inner scroll regions use
//     overflow:auto. Generous padding + large type so it reads as a real, designed panel — not a list.
//
// MOVED here from features/admin/setup/templateGallery.ts so BOTH the setup wizard and the reports
// PanelPicker share one source of truth (the cell builders in demoCells.ts consume this gallery).
// One responsibility per file (FILE-LAYOUT): the gallery data (id, label, description, sql, code). No
// markup rendering, no verbs.

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
//    ROUNDed in SQL because the template can't format. `pct` is the bar width; `rnk` is the position.
//
//    NO CTE: the host's schema/parse pass resolves every FROM/JOIN name against the catalog, and a CTE
//    name isn't a real table there ("no such table: per_site"). So the window functions run DIRECTLY
//    over the GROUP BY — `MAX(SUM(value)) OVER ()` is the leader total (the bar denominator) and
//    `RANK() OVER (ORDER BY SUM(value) DESC)` is the position. Same `federation.query` dialect (the
//    `now() - INTERVAL` filter) the shipped DEMO_SQL uses. ──
const SUMMARY_SQL = `SELECT
  s.name                                                         AS site,
  ROUND(SUM(r.value), 1)                                         AS total_kwh,
  ROUND(MAX(r.value), 2)                                         AS peak_kwh,
  ROUND(AVG(r.value), 2)                                         AS avg_kwh,
  ROUND(100.0 * SUM(r.value) / MAX(SUM(r.value)) OVER (), 0)     AS pct,
  RANK() OVER (ORDER BY SUM(r.value) DESC)                       AS rnk
FROM point_reading r
JOIN point     p ON r.point_id = p.id
JOIN meter     m ON p.meter_id = m.id
JOIN site      s ON m.site_id  = s.id
JOIN point_tag u ON u.point_id = p.id AND u.tag = 'unit' AND u.val = 'kWh'
WHERE CAST(r.time AS TIMESTAMP) >= now() - INTERVAL '4 days'
GROUP BY s.name
ORDER BY total_kwh DESC;`;

// ── Example 1: the "top consumer" spotlight leaderboard. A hero card for the #1 site (big number, a
//    conic-gradient ring drawn in CSS, its share), then a ranked list with inline bar fills. ──
const LEADER_CODE = `<div style="display:flex;flex-direction:column;gap:18px;height:100%;box-sizing:border-box;padding:20px;color:hsl(var(--fg));font-family:inherit">
  <div style="display:flex;align-items:center;gap:20px;padding:22px 24px;border-radius:20px;background:linear-gradient(135deg,hsl(var(--accent)/0.20),hsl(var(--accent)/0.04));border:1px solid hsl(var(--accent)/0.30);box-shadow:0 8px 30px -12px hsl(var(--accent)/0.45)">
    <div style="width:76px;height:76px;flex:none;border-radius:20px;display:flex;align-items:flex-end;justify-content:center;gap:5px;padding:20px 18px;box-sizing:border-box;background:hsl(var(--accent)/0.16);border:1px solid hsl(var(--accent)/0.35)">
      <span style="width:8px;height:40%;border-radius:3px;background:hsl(var(--accent)/0.45)"></span>
      <span style="width:8px;height:70%;border-radius:3px;background:hsl(var(--accent)/0.7)"></span>
      <span style="width:8px;height:100%;border-radius:3px;background:hsl(var(--accent))"></span>
    </div>
    <div style="min-width:0;flex:1">
      <div style="font-size:11px;font-weight:700;letter-spacing:0.1em;text-transform:uppercase;color:hsl(var(--accent))">Top energy consumer · last 4 days</div>
      <div style="display:flex;align-items:baseline;gap:8px;margin-top:8px">
        <span style="font-size:44px;font-weight:800;line-height:1;letter-spacing:-0.02em;font-variant-numeric:tabular-nums;color:hsl(var(--fg))">{{rows.0.total_kwh}}</span>
        <span style="font-size:16px;font-weight:600;color:hsl(var(--muted))">kWh</span>
      </div>
      <div style="margin-top:6px;font-size:17px;font-weight:600;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">{{rows.0.site}}</div>
    </div>
  </div>
  <div style="font-size:11px;font-weight:700;letter-spacing:0.1em;text-transform:uppercase;color:hsl(var(--muted));padding:0 2px">Full ranking</div>
  <div style="display:flex;flex-direction:column;gap:10px;overflow-y:auto;padding-right:4px;flex:1">
    {{#each rows}}<div style="position:relative;display:flex;align-items:center;gap:14px;padding:13px 16px;border-radius:14px;border:1px solid hsl(var(--border));background:hsl(var(--panel));overflow:hidden">
      <div style="position:absolute;left:0;top:0;bottom:0;width:{{pct}}%;background:linear-gradient(90deg,hsl(var(--accent)/0.16),hsl(var(--accent)/0.02));border-right:2px solid hsl(var(--accent)/0.35)"></div>
      <div style="position:relative;width:34px;height:34px;flex:none;border-radius:11px;display:flex;align-items:center;justify-content:center;font-size:15px;font-weight:800;background:hsl(var(--accent)/0.16);color:hsl(var(--accent))">{{rnk}}</div>
      <div style="position:relative;flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;font-size:15px;font-weight:600">{{site}}</div>
      <div style="position:relative;font-size:16px;font-weight:800;font-variant-numeric:tabular-nums">{{total_kwh}}<span style="font-size:11px;font-weight:600;color:hsl(var(--muted));margin-left:3px">kWh</span></div>
    </div>{{/each}}
  </div>
</div>`;

// ── Example 2: the KPI stat row. Big, gradient stat tiles with CSS-drawn monochrome marks (no emoji),
//    plus a native <details> "Show all sites" disclosure — a real, JS-free toggle the pure {{…}} engine
//    can't otherwise express (there is no {{#if}}). Uses the top row's fields + rows.length + iterates
//    for the expanded breakdown. ──
const STATS_CODE = `<div style="display:flex;flex-direction:column;gap:16px;height:100%;box-sizing:border-box;padding:20px;overflow-y:auto;color:hsl(var(--fg));font-family:inherit">
  <div style="display:flex;align-items:flex-end;justify-content:space-between;gap:12px">
    <div style="min-width:0">
      <div style="font-size:19px;font-weight:800;letter-spacing:-0.01em">Energy overview</div>
      <div style="font-size:13px;color:hsl(var(--muted));margin-top:3px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">Leader · {{rows.0.site}}</div>
    </div>
    <div style="flex:none;font-size:11px;font-weight:600;color:hsl(var(--muted));padding:5px 11px;border-radius:999px;border:1px solid hsl(var(--border));background:hsl(var(--panel))">{{rows.length}} sites · 4d</div>
  </div>
  <div style="display:grid;grid-template-columns:repeat(2,1fr);gap:16px">
    <div style="position:relative;overflow:hidden;display:flex;flex-direction:column;justify-content:space-between;gap:14px;padding:20px;border-radius:20px;border:1px solid hsl(var(--accent)/0.28);background:linear-gradient(150deg,hsl(var(--accent)/0.16),hsl(var(--panel)))">
      <div style="width:46px;height:46px;border-radius:13px;display:flex;align-items:flex-end;justify-content:center;gap:4px;padding:12px 11px;box-sizing:border-box;background:hsl(var(--accent)/0.2)">
        <span style="width:6px;height:45%;border-radius:2px;background:hsl(var(--accent)/0.5)"></span>
        <span style="width:6px;height:75%;border-radius:2px;background:hsl(var(--accent)/0.75)"></span>
        <span style="width:6px;height:100%;border-radius:2px;background:hsl(var(--accent))"></span>
      </div>
      <div>
        <div style="font-size:38px;font-weight:800;line-height:1;letter-spacing:-0.02em;font-variant-numeric:tabular-nums;color:hsl(var(--accent))">{{rows.0.total_kwh}}<span style="font-size:15px;font-weight:600;color:hsl(var(--muted));margin-left:4px">kWh</span></div>
        <div style="font-size:11px;font-weight:700;letter-spacing:0.08em;text-transform:uppercase;color:hsl(var(--muted));margin-top:6px">Top site total</div>
      </div>
    </div>
    <div style="display:flex;flex-direction:column;justify-content:space-between;gap:14px;padding:20px;border-radius:20px;border:1px solid hsl(var(--border));background:hsl(var(--panel))">
      <div style="width:46px;height:46px;border-radius:13px;display:flex;align-items:center;justify-content:center;background:hsl(var(--accent)/0.12)">
        <span style="width:0;height:0;border-left:9px solid transparent;border-right:9px solid transparent;border-bottom:15px solid hsl(var(--accent))"></span>
      </div>
      <div>
        <div style="font-size:38px;font-weight:800;line-height:1;letter-spacing:-0.02em;font-variant-numeric:tabular-nums">{{rows.0.peak_kwh}}<span style="font-size:15px;font-weight:600;color:hsl(var(--muted));margin-left:4px">kWh</span></div>
        <div style="font-size:11px;font-weight:700;letter-spacing:0.08em;text-transform:uppercase;color:hsl(var(--muted));margin-top:6px">Peak reading</div>
      </div>
    </div>
    <div style="display:flex;flex-direction:column;justify-content:space-between;gap:14px;padding:20px;border-radius:20px;border:1px solid hsl(var(--border));background:hsl(var(--panel))">
      <div style="width:46px;height:46px;border-radius:13px;display:grid;grid-template-columns:1fr 1fr;gap:4px;place-content:center;padding:12px;box-sizing:border-box;background:hsl(var(--accent)/0.12)">
        <span style="width:9px;height:9px;border-radius:2px;background:hsl(var(--accent))"></span>
        <span style="width:9px;height:9px;border-radius:2px;background:hsl(var(--accent)/0.55)"></span>
        <span style="width:9px;height:9px;border-radius:2px;background:hsl(var(--accent)/0.55)"></span>
        <span style="width:9px;height:9px;border-radius:2px;background:hsl(var(--accent))"></span>
      </div>
      <div>
        <div style="font-size:38px;font-weight:800;line-height:1;letter-spacing:-0.02em;font-variant-numeric:tabular-nums">{{rows.length}}</div>
        <div style="font-size:11px;font-weight:700;letter-spacing:0.08em;text-transform:uppercase;color:hsl(var(--muted));margin-top:6px">Sites reporting</div>
      </div>
    </div>
    <div style="display:flex;flex-direction:column;justify-content:space-between;gap:14px;padding:20px;border-radius:20px;border:1px solid hsl(var(--border));background:hsl(var(--panel))">
      <div style="width:46px;height:46px;border-radius:13px;display:flex;align-items:center;justify-content:center;position:relative;background:hsl(var(--accent)/0.12)">
        <span style="width:22px;height:2px;border-radius:2px;background:hsl(var(--accent)/0.5)"></span>
        <span style="position:absolute;width:9px;height:9px;border-radius:50%;background:hsl(var(--accent))"></span>
      </div>
      <div>
        <div style="font-size:38px;font-weight:800;line-height:1;letter-spacing:-0.02em;font-variant-numeric:tabular-nums">{{rows.0.avg_kwh}}<span style="font-size:15px;font-weight:600;color:hsl(var(--muted));margin-left:4px">kWh</span></div>
        <div style="font-size:11px;font-weight:700;letter-spacing:0.08em;text-transform:uppercase;color:hsl(var(--muted));margin-top:6px">Top site avg</div>
      </div>
    </div>
  </div>
  <details style="border:1px solid hsl(var(--border));border-radius:14px;background:hsl(var(--panel))">
    <summary style="cursor:pointer;list-style:none;padding:12px 16px;font-size:12px;font-weight:700;letter-spacing:0.04em;text-transform:uppercase;color:hsl(var(--accent));user-select:none">Show all sites ▾</summary>
    <div style="display:flex;flex-direction:column;gap:2px;padding:2px 8px 10px">
      {{#each rows}}<div style="display:flex;align-items:center;gap:10px;padding:7px 8px;border-radius:8px;font-size:13px">
        <span style="width:20px;color:hsl(var(--muted));font-weight:700;font-variant-numeric:tabular-nums">{{rnk}}</span>
        <span style="flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">{{site}}</span>
        <span style="font-weight:700;font-variant-numeric:tabular-nums">{{total_kwh}}</span>
        <span style="width:40px;text-align:right;color:hsl(var(--accent));font-weight:700;font-variant-numeric:tabular-nums">{{pct}}%</span>
      </div>{{/each}}
    </div>
  </details>
</div>`;

// ── Example 3: the bar-meter ranking. A dense, scannable list where each site's bar width IS its share
//    of the leader (the `pct` column). Clean, big, and impossible to get from a stock bar chart tile
//    with this exact styling. ──
const RANKING_CODE = `<div style="display:flex;flex-direction:column;gap:16px;height:100%;box-sizing:border-box;padding:20px;color:hsl(var(--fg));font-family:inherit">
  <div style="display:flex;align-items:center;gap:10px;font-size:17px;font-weight:800;letter-spacing:-0.01em">
    <span style="width:11px;height:11px;border-radius:50%;background:hsl(var(--accent));box-shadow:0 0 0 4px hsl(var(--accent)/0.2)"></span>
    Energy by site
    <span style="margin-left:auto;font-size:11px;font-weight:600;color:hsl(var(--muted))">share of leader</span>
  </div>
  <div style="display:flex;flex-direction:column;gap:18px;overflow-y:auto;padding-right:4px;flex:1">
    {{#each rows}}<div style="display:flex;flex-direction:column;gap:8px">
      <div style="display:flex;align-items:baseline;gap:10px">
        <span style="width:22px;flex:none;font-size:12px;font-weight:800;color:hsl(var(--muted));font-variant-numeric:tabular-nums">{{rnk}}</span>
        <span style="flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;font-weight:600;font-size:15px">{{site}}</span>
        <span style="font-variant-numeric:tabular-nums;font-weight:800;font-size:15px">{{total_kwh}}<span style="font-size:11px;font-weight:600;color:hsl(var(--muted));margin-left:3px">kWh</span></span>
        <span style="width:44px;text-align:right;font-size:12px;font-weight:700;color:hsl(var(--accent));font-variant-numeric:tabular-nums">{{pct}}%</span>
      </div>
      <div style="height:14px;border-radius:999px;background:hsl(var(--border)/0.5);overflow:hidden">
        <div style="height:100%;border-radius:999px;width:{{pct}}%;background:linear-gradient(90deg,hsl(var(--accent)/0.55),hsl(var(--accent)));box-shadow:0 0 12px -2px hsl(var(--accent)/0.6)"></div>
      </div>
    </div>{{/each}}
  </div>
</div>`;

// ── Example 4: draft-with-AI. A minimal-but-valid scaffold (so the preview isn't empty) that the user
//    REPLACES with agent-authored HTML: copy the AI prompt, paste the reply into the editor, preview it
//    live. The scaffold itself already binds the real fields so it renders on pick. ──
const BLANK_CODE = `<div style="display:flex;flex-direction:column;gap:16px;height:100%;box-sizing:border-box;padding:24px;color:hsl(var(--fg));font-family:inherit">
  <div style="display:flex;flex-direction:column;gap:6px;padding:20px 22px;border-radius:18px;border:1px dashed hsl(var(--accent)/0.4);background:hsl(var(--accent)/0.06)">
    <div style="font-size:11px;font-weight:700;letter-spacing:0.1em;text-transform:uppercase;color:hsl(var(--accent))">Your widget starts here</div>
    <div style="font-size:14px;color:hsl(var(--muted))">Copy the AI prompt, paste the reply into the editor, and it renders here. Or edit this markup directly.</div>
  </div>
  <div style="display:flex;flex-direction:column;gap:10px;overflow-y:auto;flex:1">
    {{#each rows}}<div style="display:flex;align-items:baseline;gap:12px;padding:12px 16px;border-radius:12px;border:1px solid hsl(var(--border));background:hsl(var(--panel))">
      <span style="flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;font-size:15px;font-weight:600">{{site}}</span>
      <span style="font-size:16px;font-weight:800;font-variant-numeric:tabular-nums">{{total_kwh}}<span style="font-size:11px;font-weight:600;color:hsl(var(--muted));margin-left:3px">kWh</span></span>
    </div>{{/each}}
  </div>
</div>`;

/** The starter widgets the Design step offers. The FIRST is the default seed; the LAST is the
 *  draft-with-AI scaffold (a minimal canvas the user replaces with agent-authored HTML). */
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
    description: "Big KPI tiles with a “Show all sites” toggle that expands the full breakdown.",
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
  {
    id: "ai",
    label: "Draft with AI",
    description: "Start from a blank canvas and paste an AI-authored widget — copy the prompt below.",
    sql: SUMMARY_SQL,
    code: BLANK_CODE,
  },
];

/** The default starter (the leaderboard) — the seed the Design step opens with. */
export const DEFAULT_TEMPLATE = TEMPLATE_GALLERY[0]!;
