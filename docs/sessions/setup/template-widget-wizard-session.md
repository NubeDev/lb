# Render-template widget setup wizard — session

Status: shipped. Added a "Build a render-template widget" wizard to the Setup tab that walks a user from
the seeded `demo-buildings` datasource → a preloaded query → designing a custom HTML/JSX widget against
their real rows (with an AI-prompt seam to draft it via any external agent) → saving it as a durable,
reusable `render_template` (and optionally onto a fresh dashboard).

## The ask

From the setup guide, let a user create a render-template widget using the seeded `demo-buildings`
datasource and the hourly-energy-per-site query; preview the template inside the guide, edit/enter new
JSX, and copy an AI prompt so they can have any external agent draft one. See
`docs/scope/admin/setup/setup-wizards-scope.md` + `docs/scope/frontend/dashboard/render-template-widget.md`.

## The mental model the wizard teaches

1. **Datasource — where the data lives.** The registered `demo-buildings` SQLite dataset.
2. **Query — the rows to render.** The preloaded `DEMO_SQL` (hourly avg energy per site, 4 days). Run
   it; the rows it returns are exactly what the widget renders + what the AI prompt samples.
3. **Design — write the widget.** The inline template HTML/JSX editor beside a live preview, bound to
   the query rows. Engine is pure `{{…}}` interpolation (no JavaScript).
4. **Ask an AI — draft it for you.** Copy the engine contract + real sample rows + the SQL; paste any
   LLM's reply back into the editor.
5. **Save — a reusable widget.** Persist as a durable `render_template` (`template.save`); optionally
   drop the same cell on a new dashboard (`dashboard.save`).

## Design decisions (reuse-first)

- **Sibling of the data→insight wizard.** Steps 1–2 are IDENTICAL to `DatasourceWizard`, so per setup
  rule 3 ("extract, don't fork") I **extracted** the Datasource + SQL step bodies into shared
  `FlowStep` components (`steps/DatasourceStep.tsx`, `steps/SqlPreviewStep.tsx`) that BOTH wizards now
  import. No copy; one implementation, two homes. `DatasourceWizard` was updated to consume them (its
  local `DatasourceStep`/`SqlStep` deleted) and its existing gateway test stays green.
- **The SQL step gained one seam.** The template wizard needs the ran rows (to feed the preview + the AI
  prompt), so `SqlPreviewStep` optionally reports them via `onRows` on each successful run. Default
  behavior (data→insight) is unchanged.
- **No forked editor, no forked preview.** The Design step renders the shipped `TemplateSourceField`
  (inline mode) + a live `WidgetHost` over a `view:"template"` cell — the SAME in-process `TemplateView`
  the dashboard uses. What the user designs is a real cell; what they save is the same `code`, so there
  is no drift between preview and saved artifact.
- **The AI prompt is the shipped one.** `CopyTemplatePrompt` + `buildTemplatePrompt` already emit the
  engine contract + the user's real rows + the SQL. Mounted inside a `ResultRowsProvider` fed with the
  ran rows — zero new prompt code.
- **`templateCell` mirrors `timeseriesCell`.** Added to `dataToInsight.ts` beside its sibling: same
  `federation.query` source binding, but `view:"template"` + `options.code` (the key `TemplateView`
  reads). Plus `TEMPLATE_STARTER`, a per-row energy table over `DEMO_SQL`'s columns.
- **Cap gate.** The caps map exposes no `template.save` constant; `template.save`/`dashboard.save` share
  the widget-authoring trust class (render-template scope), so `CAP.dashboardSave` gates both the durable
  save and the dashboard drop for DISPLAY. The gateway re-checks every write (rule 5).

## Reuse ledger

| Step | Reused from (component / hook / verb) | New code written? |
|---|---|---|
| Overview | — (explanatory copy) | intro copy in `TemplateWidgetWizard.tsx` only |
| Datasource | **extracted** shared `steps/DatasourceStep` (rule 3) over `useDatasourceList` + `datasource.list`/`datasource.add` | shared step file (moved, not forked) |
| Query | **extracted** shared `steps/SqlPreviewStep` over `useQueryRun` + `QueryResults` + `federation.query`; `CodeEditor` (read-only); `DEMO_SQL` | shared step file + one `onRows` seam |
| Design | `dashboard/builder/editors/TemplateSourceField` (inline editor) + `dashboard/WidgetHost` → `TemplateView` (live preview); `templateCell` | `templateCell()` + `TEMPLATE_STARTER` in `dataToInsight.ts` |
| Ask an AI | `panel-builder/CopyTemplatePrompt` + `buildTemplatePrompt`; `ResultRowsProvider`/`useResultRows` | none |
| Save | `template.save` (`saveTemplate`) + optionally `dashboard.save` (`saveDashboard`); `templateCell` | thin save step in the flow file |

No new backend, no new verb, no duplicated editor. Datasource/source ids stay opaque (rule 10 — no
branch on a named extension); the wizard operates only in the session workspace (rule 6).

## Making the widgets actually look good (the starter gallery)

The first cut shipped one cramped starter and it looked bad. The rebuild adds a **gallery of three
polished starter widgets** (`templateGallery.ts`) the Design step offers as cards — pick one and it
seeds the editor + re-points the query. All three are genuinely designed (big numbers, gradient hero,
icon-badged KPI tiles, glowing bar meters), verified by screenshotting the real sanitized output in
both light and dark themes with real `demo-buildings` rows.

Two hard constraints the rebuild had to honor (both discovered by rendering, not by reading):

1. **The sanitizer STRIPS `<style>` blocks.** DOMPurify drops `<style>` contents entirely, so a
   class-based stylesheet renders as unstyled stacked text (the original bug). Every element now carries
   an **inline `style=""`** attribute, and a bar width is a literal `width:{{pct}}%` — CSS custom
   properties (`--p`) are useless without a stylesheet. `templateGallery.test.ts` asserts `style=`
   survives and `<style` does not, so this can't regress.
2. **No CTE in the query.** The host's schema/parse pass resolves every FROM/JOIN name against the
   catalog, so a `WITH per_site AS (…)` failed with `no such table: per_site`. The shared `SUMMARY_SQL`
   runs the window functions (`MAX(SUM(value)) OVER ()` for the bar denominator, `RANK() OVER (…)` for
   position) **directly over the GROUP BY** — no CTE. Verified end-to-end against the live dev node via
   `POST /mcp/call federation.query` (8 sites, `pct`/`rnk` computed).

The three examples (each answers "a view we don't already have pre-made"): **Top consumer spotlight**
(hero card for #1 + ranked list with inline share bars), **Energy stat tiles** (big KPI tiles), and
**Bar-meter ranking** (labelled progress bars sized by share of the leader).

Follow-up polish (from live review of the rendered widgets):

- **CSS-drawn monochrome icons, not emoji.** Color emoji (⚡🏆📈) looked cheap and ignore the theme, so
  the marks are drawn with divs (a bar-chart glyph, a CSS-border triangle, a 2×2 grid, a baseline dot)
  tinted `hsl(var(--accent))` — crisp in both themes.
- **A real interactive toggle.** The stats example carries a native `<details>`/`<summary>` "Show all
  sites" disclosure — a JS-free expander the pure `{{…}}` engine can't otherwise express (there is no
  `{{#if}}`), and `<details>` survives the sanitizer. (A `data-call` button is for host-mediated WRITES,
  not a client-side view toggle, so it's the wrong tool for "show more"; `<details>` is the right one.)
- **Code hidden by default in the wizard.** The Design step collapses the JSX editor behind an
  "Edit code" / "Hide code" toggle so the polished preview takes the full width (taller when code is
  hidden) — a designed widget should read as a widget, not a wall of markup.
- **"Ask an AI" folded INTO the Design step (no separate step 5).** The point of the AI prompt is to
  draft a widget and preview it, so the Copy-AI-prompt button now lives on the Design toolbar next to
  the editor + live preview — copy the prompt, paste the agent's HTML into the editor, see it render.
  A 4th gallery card, **"Draft with AI"**, seeds a minimal accent-dashed canvas (already binding the
  real fields, so the preview isn't empty) and auto-opens the editor as the paste target. The shared
  `buildTemplatePrompt` was tuned to produce a *polished, big* widget on this data: it now forbids
  `<style>`/SVG explicitly (both are stripped), pushes large hero numbers + generous padding + rounded
  cards + accent highlight, and points at `<details>` for a JS-free toggle. Wizard is now 4 real steps:
  Datasource → Query → Design (with AI + preview) → Save.

## Files touched

- `ui/src/features/admin/setup/steps/DatasourceStep.tsx` — **new** (rule-3 extraction, shared).
- `ui/src/features/admin/setup/steps/SqlPreviewStep.tsx` — **new** (rule-3 extraction, shared; `onRows` seam).
- `ui/src/features/admin/setup/DatasourceWizard.tsx` — consume the extracted steps (local copies deleted).
- `ui/src/features/admin/setup/dataToInsight.ts` — add `templateCell()`.
- `ui/src/features/admin/setup/templateGallery.ts` — **new**: the three polished starter widgets +
  the shared CTE-free summary SQL.
- `ui/src/features/admin/setup/templateGallery.test.ts` — **new**: interpolate+sanitize each example
  (no leftover tokens, inline styles survive, bar width lands).
- `ui/src/features/admin/setup/TemplateWidgetWizard.tsx` — **new** wizard (6 steps: intro + 5 real),
  with the Design-step gallery picker + taller live preview.
- `ui/src/features/admin/setup/TemplateWidgetWizard.gateway.test.tsx` — **new** real-gateway test.
- `ui/src/features/admin/setup/catalog.ts` — add the `template` entry (`LayoutTemplate` icon).
- `ui/src/features/admin/setup/SetupHub.tsx` — add the `template` branch inside `DashboardCacheProvider`.

## Tests (real gateway, no fakes — CLAUDE §9)

`TemplateWidgetWizard.gateway.test.tsx` drives the wizard against a real seeded gateway and asserts the
**real write effects**, plus the mandatory deny + isolation categories (testing-scope §2):

- **Full flow** — register the demo (`datasource.list` reads it back) → run the query → the Design step
  mounts the real `TemplateView` preview + inline editor → Save lands a real `render_template`
  (`getTemplate` reads back `engine:"template"` + the `{{#each rows}}` body) → "Add to a new dashboard"
  lands a real `dashboard.save` with a `view:"template"` cell (read back over the gateway).
- **Cap deny** — a session without `dashboard.save` sees the save controls hidden AND the host refuses
  `template.save` (opaque reject — the backstop, not the hiding).
- **Workspace isolation** — a `render_template` saved in ws-A is invisible in ws-B (`template.get` →
  throws; `template.list` omits it).

```
✓ src/features/admin/setup/TemplateWidgetWizard.gateway.test.tsx (3 tests)   # new
✓ src/features/admin/setup/DatasourceWizard.gateway.test.tsx (1 test)        # unchanged by the extraction
✓ src/features/admin/setup/IngestWizard.gateway.test.tsx (1 test)            # sibling unaffected
✓ src/features/admin/setup/AgentWizard.gateway.test.tsx (1 test)             # sibling unaffected
```

`pnpm exec tsc --noEmit` clean; `eslint` clean on the new source files. A fresh `nextWs()` per test
isolates the shared node. (jsdom emits a benign `getClientRects` warning from CodeMirror's measurement
in the Design step — the same editor the data→insight wizard mounts; not a failure.)
