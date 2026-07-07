---
name: render-widgets
description: >-
  Answer with a LIVE built-in widget. If the user says jsx/html/custom/template, the view is
  view:"template" with inline options.code HTML — NOT a named view. Otherwise pick a named view —
  stat, chart, gauge, table, timeseries, barchart, piechart — by
  writing a `rich_result` render envelope as a fenced ```lb-widget code block INSIDE your final answer
  text. Pick the view + its option keys from dashboard.catalog (the source of truth for what is
  stylable), bind source to a query you PROVED, and the host splits the block off and renders it through
  the SAME widget renderer dashboards use — no `channel.post`, no `dashboard.save`. The user can then
  pin it to a dashboard (dashboard.pin) as a durable widget in one click.
---

# render-widgets — answer with a live built-in widget

## ⚡ ROUTE FIRST — which view? (decide BEFORE anything else)

If the user's ask contains **"render widget"**, **"render template"**, **"jsx"**, **"html"**,
**"template"**, **"custom"**, **"markup"**, or "make it look cool / branded / your own layout" →
the answer is **`view:"template"`** with inline `options.code` HTML. On this platform "render
widget" IS the template feature — that phrase alone routes here. **Do NOT pick a named view (barchart/table/stat/…) for these asks — a named
view is the WRONG answer even if the data would fit one.** Jump to
[Render templates](#render-templates-viewtemplate--custom-htmljsx) and follow its workflow.

Only when the user names a standard visualization ("a stat", "a gauge", "a bar chart of …", "a
table of …") or asks generically ("show me X") do you pick a named view from the catalog.

**When this skill applies** — the user says any of: "make me a render widget", "use a render-widget",
"make me a stat widget", "show me a chart/gauge/table of …", "give me a widget with an arg like X",
"make a widget using jsx/html/template", or any ask for ONE built-in visualization (a single number,
a trend, a gauge, rows, a pie, a bar comparison, OR custom HTML/JSX-style markup). It also applies
any time you would otherwise reach for `view:"genui"` for a SINGLE visual — a single stat is
`view:"stat"`, NOT a `view:"genui"` with one Stat component. Reach for `view:"genui"` ONLY when the
user asks for a COMPOSED card (a stat AND a chart AND a table in one surface).

**Two kinds of built-in widget you can author**, both delivered as a fenced ```lb-widget block:

1. **A named view** (`stat`, `timeseries`, `gauge`, `table`, `barchart`, `piechart`, `bargauge`,
   `histogram`) — the platform renders it; you pick the view + option keys from `dashboard.catalog`.
   Use this for any standard visualization.
2. **A render template** (`view:"template"` with inline `options.code`) — YOU write the HTML/JSX-style
   markup, the platform interpolates your query rows into it and sanitizes it. Use this when the user
   asks for "jsx", "html", "custom layout", "make it look cool", or a visualization the named views
   can't express (a card grid, a custom stat tile, branded chrome). See
   [Render templates](#render-templates-viewtemplate--custom-htmljsx) below.

When the user asks for a single number ("average session time"), a chart, a gauge, a table, or any
**built-in** visualization, write the widget **as a fenced ```lb-widget block inside your final answer
text**. The host worker (which owns the conversation's channel id) splits the block off, strips it
from your persisted prose answer, and posts it as a `rich_result` item that the dock renders live —
re-fetching the bound `source` on every view, so the widget stays true to the store, not a snapshot.

**You do NOT call `channel.post`. You do NOT need to know the channel id. Just write the block.**

This is the SAME choreography [channel-widgets](../channel-widgets/SKILL.md) documents for `genui`
layouts — the no-`channel.post` dock path is GENERIC over the `rich_result` view. Reach for **this**
skill (a built-in view) by default; reach for `view:"genui"` ONLY when the user asks for a *composed*
card (a stat AND a chart AND a table in one surface).

## ⚠ The fenced block IS the deliverable — do not stop after discovery

The widget is the answer. Discovery (`dashboard.catalog`) and proof (`store.query` /
`federation.query`) are PREP — they feed the binding. The run is NOT done until your FINAL turn's
text contains the fenced ```lb-widget block. A run that does discovery then stops with only a
preamble ("I'll create a widget…") or an empty `done` has FAILED the request — the user gets no
widget, just the preamble you wrote before the tool calls. Concretely:

- After your last tool call returns, your NEXT turn MUST be the final answer, and that answer MUST
  contain the fenced block. Do not end the run with an empty turn. Do not describe the widget you
  "will" build — BUILD it, inline, as the block, in the same turn you mark done.
- The fenced block is not a side-effect or a follow-up; it lives inside your answer prose. The host
  splits it off and renders the widget; the surrounding prose is what the user reads. Both ship in
  ONE final turn.
- If you find yourself about to stop with text like "Let me build that for you" or "I'll now create
  the widget" — STOP. You already have the catalog + the proven query. Emit the block NOW, in this
  turn, as your answer.

## The choreography (catalog, prove, then embed)

1. **Discover the palette + style keys.** Call `dashboard.catalog` (cheap, member-level read). It
   returns `{ v, views, extWidgets, genuiComponents }`. `views[]` is the source of truth for which
   built-in views exist (`stat`, `timeseries`, `gauge`, `table`, `barchart`, `piechart`, `bargauge`,
   `histogram`, …), what `kind` each is (`viz`/`control`/…), whether it takes data (`data:true`), and
   — critically — the **per-view `options[]` list** that names every stylable key, its `scope`
   (`"fieldConfig"` vs `"options"`), its `path`, its `control`, and its `choices`. **Do not guess a
   view name or an option key** — both come from the catalog. Reading the catalog grants knowledge,
   nothing else.
2. **Prove the data first.** For a `federation.query` source, call **`federation.schema`
   { source } FIRST** and read the real table + column names — never guess a join or a column;
   a guessed schema fails the query and derails the run. Then run the EXACT `{tool, args}` you
   intend to bind as the `source` —
   `store.query { sql }`, `federation.query { source, sql }`, `series.latest { series }`,
   `series.read { series, range }`, `viz.query`, … — and confirm non-empty rows with the columns you
   expect. **A schema read is NOT proof — you must RUN the query and see rows.** Federation SQL
   dialect gotcha: a window function over an aggregate in the SAME select
   (`AVG(x) * 100 / MAX(AVG(x)) OVER ()`) fails — compute the aggregates in an inner subquery
   first, then window/derive over the subquery's plain columns in the outer select. An envelope whose
   SQL you never executed WILL render "no rows" in the dock (the exact failure this rule exists to
   prevent). If a column lives in another table (e.g. `site` names when readings only carry
   `point_id`), write the JOINs the schema shows. An unproven binding is a dead widget. Pick the view that matches the data shape: a single
   number → `stat` (or `gauge` with a threshold); a value over time → `timeseries`; rows → `table`;
   parts of a whole → `piechart`; categories compared → `barchart`.
3. **Write your final answer with the widget embedded.** Put prose for the user, then a fenced
   ```lb-widget block carrying the `rich_result` envelope JSON, then any closing prose. The block is
   removed from what the user reads in your text answer; the widget renders as its own card right
   below. A worked example — "average session time, in seconds, as a stat":

```
Here is the average session time across the last 24 h:

```lb-widget
{
  "kind": "rich_result",
  "v": 2,
  "view": "stat",
  "source": { "tool": "store.query",
              "args": { "sql": "SELECT duration_s AS value FROM session WHERE ended_at > time::now() - 24h" } },
  "options": { "reduceOptions": { "calcs": ["mean"], "fields": ["value"] },
               "textMode": "auto", "colorMode": "value" },
  "fieldConfig": { "defaults": { "unit": "s" } },
  "tools": ["store.query"]
}
```

Want me to pin this to a dashboard?
```

4. **One widget per answer.** The first valid ```lb-widget block wins; later ones are left in your
   text. If the user wants a second widget, they will ask again. To compose MULTIPLE visuals into ONE
   card, that is the `view:"genui"` path (channel-widgets skill) — not two fenced blocks.

## Envelope fields

- `kind` — always `"rich_result"`.
- `v` — always `2` (the envelope version).
- `title` — optional; names the widget. At pin time it becomes the dashboard cell's title AND the
  reusable panel's name (`panel.list`), so set a short human title ("Site Energy Ranking") — the
  user can also edit it in the pin dialog.
- `view` — a built-in view id from `dashboard.catalog` (`stat`, `timeseries`, `gauge`, `table`,
  `barchart`, `piechart`, `bargauge`, `histogram`, …). Use `genui` ONLY for a composed layout
  (channel-widgets skill).
- `source` — the `{tool, args}` the viewer re-runs to load data. This is what makes the widget LIVE
  and honest: bind the exact proven query. Never inline `data` instead of a source — an inline-only
  envelope has no read path and degrades.
- `tools` — list every tool the `source` (and any `action`) names. The host intersects it with the
  viewer's own grant on every load, so you can never widen what a viewer may read.
- `options` / `fieldConfig` — optional presentation, keyed by the catalog. See "Styling / icons"
  below.

## Args / variables ("takes an arg like user session time")

A widget that "takes an arg" (per-user session time, per-site avg power, a date range) binds the arg
**through the source** — the SQL/args carry the concrete value for the PREVIEW. Author a concrete
binding; do not invent a placeholder/template syntax the host does not run.

- For a preview scoped to ONE user (the one who asked), seed the proven query with their identity
  (`session.user = $caller` style is NOT supported — bind the literal the user is asking about, or
  scope the SQL to a value the user named).
- For a range the user gave you ("last 24 h"), bake that range into the SQL.
- The user edits the parameterization LATER via the data studio once the panel is pinned — the
  panel's `panel_vars` is the system for dashboard-level variables. For the PREVIEW, author a concrete
  binding; an unbound `${var}` in the source SQL is a dead widget today.

If the user asks for "make it parameterizable", say that dashboard-level variables (`panel_vars`) are
set in the data studio AFTER pinning, and pin a working concrete preview first so they have something
to parameterize.

## Styling / icons (the catalog is the source of truth)

The catalog's per-view `options[]` is the authoritative list of what is stylable on each view —
**consult it; do not enumerate keys from memory.** Each entry carries:

- **`id`** + **`path`** — the option's stable id and dotted path (`unit`, `min`, `max`,
  `legend.showLegend`, `custom.showPoints`, `reduceOptions.calcs`, `textMode`, `colorMode`, …).
- **`scope`** — WHERE on the cell it lives: `"fieldConfig"` → under
  `fieldConfig.defaults.<path>`; `"options"` → under `options.<path>`. Honoring `scope` is the
  single most common authoring mistake — a `unit` set under `options` instead of
  `fieldConfig.defaults.unit` renders as a default. Place each value at its scope.
- **`control`** + **`choices`** — input kind; for a `select`, the value MUST be one of `choices`.

**Icons.** Stat and gauge views take a `title` and a `fieldConfig.defaults.displayName` for labelling.
For a "nice icon" request, set `fieldConfig.defaults.displayName` and a `title` on the cell; the
shell renders a lucide icon name where the catalog's `icon` control is present (ext tiles expose one;
built-in views currently expose labelling/title, not a per-cell icon picker — tell the user the
truth if no icon control exists for the view they picked, and lean on labelling + tone instead).
Threshold colors (`fieldConfig.defaults.thresholds`) and `colorMode` (stat) / `tone` props carry the
"looks cool" feel: a threshold flips the value red past a limit; a `colorMode:"background"` paints the
whole stat panel.

Set options by their catalog ids and the cell renders as intended; invent keys and the host accepts
the envelope but the option silently no-ops at view time. The catalog exists precisely so you do not
rely on the host to catch a bad option.

## Capabilities

You need NO special capability to write the widget block — the host worker posts the envelope under
its OWN authority (the conversation channel is the run's own). Viewers load the `source` under their
OWN grant: a viewer without the read cap sees the standard denied state, never your data. Reading the
palette needs `mcp:dashboard.catalog:call` (member-level); binding a source needs that source's own
read cap (`mcp:store.query:call`, `mcp:federation.query:call`, `mcp:series.read:call`, …) — checked
again at render under the viewer. If you prove the data first (step 2), the capability flow is
already exercised.

## Pin behavior (same as the genui path)

The rendered item carries a **pin** affordance in the shell; headless, the same envelope (minus
`kind`/`v`) is the `dashboard.pin` argument:
`dashboard.pin { dashboard, title?, now, envelope: { view, source, options?, fieldConfig?, tools } }`.
The pin mints a persisted dashboard cell that renders identically AND saves the widget as a reusable
`panel:{slug}` record (the widget library) attached by reference — so the user can later drop the
same widget onto other dashboards or open it in the data studio. Offer this when the user says
"keep", "save", "pin", or "add to a dashboard".

## Render templates (view:"template") — custom HTML/JSX

When the user asks for "jsx", "html", a "custom layout", "make it look cool", or any visualization
the named views (`stat`/`chart`/`gauge`/…) can't express, author a **render template**: a
`view:"template"` envelope whose `options.code` is HTML you write, with the platform interpolating
your proven query rows into it. This is the closest thing to JSX the platform has — inline HTML
markup, data-bound, sanitized, no JavaScript. The same fenced-block delivery applies:

````
```lb-widget
{
  "kind": "rich_result", "v": 2, "view": "template",
  "source": { "tool": "federation.query",
              "args": { "source": "demo-buildings",
                        "sql": "SELECT s.name AS site, ROUND(AVG(r.value), 1) AS avg_kw FROM point_reading r JOIN point p ON p.id = r.point_id JOIN meter m ON m.id = p.meter_id JOIN site s ON s.id = m.site_id GROUP BY s.name ORDER BY avg_kw DESC LIMIT 8" } },
  "options": { "code": "<div style=\"padding:8px\"><h3 style=\"font-size:12px;color:hsl(var(--muted));margin:0 0 6px\">Top sites by avg kW</h3><ul style=\"list-style:none;margin:0;padding:0\">{{#each rows}}<li style=\"display:flex;justify-content:space-between;padding:4px 0;border-bottom:1px solid hsl(var(--border))\"><span style=\"font-size:11px;color:hsl(var(--fg))\">{{site}}</span><span style=\"font-size:11px;font-variant-numeric:tabular-nums;color:hsl(var(--accent));font-weight:600\">{{avg_kw}} kW</span></li>{{/each}}</ul></div>" },
  "tools": ["federation.query"]
}
```
````

### Authoring workflow (QUERY FIRST, always)

You cannot write a correct template against columns you haven't seen. For a federation source,
call **`federation.schema { source }` FIRST** — it returns the real tables + columns; write the SQL
against those names only. If the query errors, FIX the SQL against the schema — do **not** fall back
to synthetic inline rows (`SELECT * FROM [{...}]`); a widget over made-up data is a failed answer
when the user asked for a real datasource. Then **run the source query**
(`federation.query` / `store.query`) and read the actual rows + column names, THEN author the HTML
binding those exact field names. A template that guesses a column name (`{{avg_consumption}}` when
the query returned `avg_kw`) renders empty cells. Derive any computed value IN THE SQL
(`SELECT AVG(value) AS avg_kw`, `SELECT value * 1.2 AS adjusted`, …) — the template engine has NO
math, no formatting helpers, no conditionals.

### The template engine (strict — anything else will not render)

- **NO JavaScript at all**: no `<script>`, no event handlers (`onclick`, `onerror`, …), no
  expressions. The output is sanitized with DOMPurify before it touches the DOM; a script tag or
  inline handler is stripped silently. The engine is pure interpolation.
- **Bindings** (Mustache-style, eval-free):
  - `{{rows.length}}` — row count.
  - `{{latest.FIELD}}` — the last row's value (good for a headline stat above a list).
  - `{{#each rows}}…{{/each}}` — iterate the rows. SINGLE LEVEL (no nested `each` — the first
    `{{/each}}` closes the block). Inside the block, `{{FIELD}}` reads that row's column and
    `{{.}}` is the whole row object.
  - `{{loading}}` / `{{denied}}` — booleans for the panel state (you cannot branch on them — there
    is no `{{#if}}`; the host shows its own loading/denied chrome, so omit them).
- **Unknown paths render empty** (never `undefined`, never crash). There is **no math, no
  conditional, no formatting** — if a value needs deriving, it must already be a column in the SQL.
  Round/aggregate/cast in SQL, not in the template.
- **Optional write buttons** (host-mediated, capability-leashed): inside the template, a
  `<button data-call="tool.name" data-args='{"k":1}'>Run</button>` routes a click through the host
  bridge to MCP `tool.name` with the given args. The tool MUST be in the envelope's `tools[]` (the
  leash); a button whose `data-call` isn't leashed is a no-op. The token never enters the template.

### Styling (the widget must look native in the host app)

- **INLINE `style` attributes only** — utility CSS classes (Tailwind shorthand like `flex`,
  `text-sm`) WILL NOT EXIST at runtime inside the template's `innerHTML`. Spell every style out as
  an inline `style="..."`.
- **Use the host theme tokens** so light/dark both work — the template renders inside the shell:
  - text: `color:hsl(var(--fg))`, muted text: `color:hsl(var(--muted))`
  - accent (links, values, highlights): `color:hsl(var(--accent))`
  - surfaces: `background:hsl(var(--panel))`, borders: `border-bottom:1px solid hsl(var(--border))`
  - alpha: `background:hsl(var(--accent)/0.15)` (highlight chip)
- Small type (`font-size:10px`–`12px`), rounded corners (`border-radius:6px`), `font-variant-numeric:tabular-nums`
  for numbers so they align.
- The widget fills a dashboard panel: one flex column (`display:flex;flex-direction:column`), inner
  lists scroll (`overflow-y:auto`), nothing may overflow. Keep the whole template **under 4 KB**
  (inline `options.code` limit).

### Worked example — a stat + ranked list from one query

SQL (prove it first): `SELECT COUNT(*) AS total, AVG(value) AS avg_kw FROM point_reading` → one row.
Template body (the headline uses `latest.total` / `latest.avg_kw`; the ranked list below binds a
SECOND query's rows — emit TWO sources with `{{#each rows}}` over the ranked one):

```
<div style="display:flex;flex-direction:column;gap:8px;padding:8px">
  <div style="display:flex;justify-content:space-between;align-items:baseline">
    <span style="font-size:11px;color:hsl(var(--muted))">Readings</span>
    <span style="font-size:18px;font-weight:600;font-variant-numeric:tabular-nums;color:hsl(var(--fg))">{{latest.total}}</span>
  </div>
  <ul style="list-style:none;margin:0;padding:0;overflow-y:auto;display:flex;flex-direction:column;gap:2px">
    {{#each rows}}<li style="display:flex;justify-content:space-between;padding:3px 0;border-bottom:1px solid hsl(var(--border))">
      <span style="font-size:11px;color:hsl(var(--fg))">{{site}}</span>
      <span style="font-size:11px;font-variant-numeric:tabular-nums;color:hsl(var(--accent));font-weight:600">{{avg_kw}} kW</span>
    </li>{{/each}}
  </ul>
</div>
```

Envelope `source` binds the query that produces `rows[]` (the ranked list); the headline reads
`{{latest.total}}` off the SAME rows (the last row). For a headline that reads a DIFFERENT query,
emit two `sources[]` targets (refIds A/B) and pick the rows you iterate — the template engine
iterates the merged `rows` array; for a single-row stat source, `{{latest.FIELD}}` is the value.

## What this skill is NOT

- Not the composed-card path — that is `view:"genui"` (channel-widgets skill). Reach for it ONLY when
  the user asks for multiple visuals in one surface.
- Not a `dashboard.save` path — a preview never writes a dashboard; only `dashboard.pin` (the user's
  explicit keep) does, and it goes through the SAME view validator (`dashboard.catalog`'s view list),
  so an envelope with a hallucinated view is rejected at pin time too.
- Not a place to invent option keys — the catalog is the source of truth for stylable fields.
