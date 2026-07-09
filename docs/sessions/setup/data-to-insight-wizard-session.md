# Data → insight setup wizard — session

Status: shipped. Added a "Data to insight" wizard to the Setup tab that walks a user the full data path
end to end — datasource → SQL → panel → dashboard → rule → insight — one page per key part, each with a
plain-language "what this is for" intro and the *real* thing running behind it.

## The ask

Make a setup wizard that: (1) picks a datasource + preloads a SQL query with a Run button, then builds a
panel and saves it to a dashboard (a timeseries with one line per site); (2) previews a rule (read-only,
syntax-highlighted) with a Run button — no editor; (3) shows the insights the rule raised and explains
how they work. "Nice UX/UI — recode where we can, but take parts from the new-panel and rules pages."
See `docs/scope/admin/setup/setup-wizards-scope.md`.

## The mental model the wizard teaches

The whole data path, in six steps:

1. **Datasource — where the data lives.** A registered connection (here the shipped `demo-buildings`
   SQLite dataset). Everything downstream reads through it.
2. **SQL — ask a question of it.** A preloaded query; press Run, see rows. Averages energy per site,
   per hour, over 4 days.
3. **Panel — draw the answer.** A timeseries, one line per site. The query returns a LONG frame (one
   row per hour × site); the panel's `plot` spec (`seriesField: "site"`) pivots it into per-site lines.
4. **Dashboard — save it somewhere.** The panel lands on a fresh, reopenable dashboard.
5. **Rule — watch it automatically.** A Rhai script re-runs a query and raises an insight when a
   building goes over budget. Previewed read-only, run once here.
6. **Insight — the durable finding.** A deduped, acknowledgeable record — the payoff.

## Design decisions (the "nice UX" part)

- **One step-rail, not two.** Embedding the whole `PanelWizard` (its own step rail) inside a wizard step
  would nest two rails. Instead the Panel/Dashboard step builds the *same* v3 timeseries `Cell` the
  panel wizard's prefill produces (`timeseriesCell`, mirroring `seedFromPrefill`) and previews it live
  through the **same `WidgetHost`** the dashboard grid uses, then saves via `dashboard.save`. One save
  button, real preview, no forked editor.
- **Split by site = the plot spec, not the query.** The timeseries renderer has two paths: a single-value
  path (collapses everything to one line) and the multi-series `PlotChart` path, taken only when the cell
  has an `options.plot` spec. A `GROUP BY site` query alone still renders ONE line — the split lives in
  `PlotSpec.seriesField: "site"`, which pivots the long (site, hour, value) frame into per-site lines.
  `timeseriesCell` sets `DEMO_PLOT` (`xField: hour`, `yFields: [avg_energy]`, `seriesField: site`); the
  gateway test asserts it persisted so the collapse-to-one-line regression can't recur.
- **SQL and rule are preloaded + read-only.** The user *runs*, doesn't author (authoring lives in the
  Query workbench / Rules page). Reused the real run engines (`useQueryRun`, `useRules`) and result
  panes (`QueryResults`, `RunResult`) verbatim; the code shows in a read-only `CodeEditor`.
- **Real demo domain.** The SQL + rule target the shipped `demo-buildings` source (the same one the
  Query workbench and the host's `buildings_examples.json` regression rule use), so every step runs
  against real data, not a fixture.

## Reuse — one extraction, no forks (setup rule 3)

The shared `CodeEditor` wrapper didn't forward CodeMirror's `editable` prop, so a read-only *preview*
wasn't possible without either a fork or CodeMirror directly. Per rule 3 I **extended** the wrapper with
an `editable` passthrough (default `true`) — now the rules editor and this preview use one component. No
other editor was touched: the SQL/rule run engines, the panel model, the dashboard verb, and the
insights widget are all reused as-is.

## Reuse ledger

| Step | Reused from (component / hook / verb) | New code written? |
|---|---|---|
| Overview | — (pure explanatory copy) | intro copy in `DatasourceWizard.tsx` only |
| Datasource | `panel-builder/tabs/useDatasourceList`; verbs `datasource.list` / `datasource.add` (`@/lib/datasources`) | the picker `<select>` (~15 lines, the SourceStep pattern) |
| SQL | `query-workbench/useQueryRun` + `datasources/QueryResults`; verb `federation.query`; `CodeEditor` (read-only) | preloaded SQL string in `dataToInsight.ts` |
| Panel & dashboard | `dashboard/WidgetHost` (live preview) + `panel-kit/defaultCell` + `viewOptions.defaultOptionsForView` + `charts.PlotSpec` (`seriesField` split) + `dashboard.save` (`@/lib/dashboard`) | `timeseriesCell()` + `DEMO_PLOT` (mirrors `PanelWizard.seedFromPrefill`) |
| Rule | `rules/useRules` (`setBuffer`+`run`→`rules.run`) + `rules/RunResult`; `CodeEditor` (read-only) + `codeLanguageExtension("rhai")` | preloaded rule string in `dataToInsight.ts` |
| Insights | `@nube/insights` `InsightsReadWidget` over the shell `insightsClient`; verb `insight.list` | none |

No new backend, no new verb, no duplicated editor. Cap-gating hides controls (rule 5 — the gateway is
the wall: `datasource.list`, `dashboard.save`, `rules.run`, `insight.list`); the datasource/source ids
stay opaque (rule 10 — no branching on a named extension).

## Files touched

- `ui/src/components/codeeditor/CodeEditor.tsx` — added `editable?` passthrough (rule-3 extraction).
- `ui/src/features/admin/setup/dataToInsight.ts` — **new**: the three preloaded strings (SQL, rule) +
  `timeseriesCell()` builder. Data + one pure helper, kept out of the flow file (FILE-LAYOUT).
- `ui/src/features/admin/setup/DatasourceWizard.tsx` — **new** wizard (6 steps).
- `ui/src/features/admin/setup/DatasourceWizard.gateway.test.tsx` — **new** real-gateway test.
- `ui/src/features/admin/setup/catalog.ts` — added the `datasource` entry (`Lightbulb` icon).
- `ui/src/features/admin/setup/SetupHub.tsx` — added the `datasource` branch, wrapped in the
  `DashboardCacheProvider` the live preview + datasource roster read through.

## Tests (real gateway, no fakes — CLAUDE §9)

`DatasourceWizard.gateway.test.tsx` drives the wizard against a real seeded gateway and asserts the
**real write effects**, not just the UI:

- **Datasource** — clicking Register lands a real `datasource.list` row (`listDatasources()` reads it
  back).
- **Dashboard** — Save lands a real dashboard; `listDashboards()` + `getDashboard()` read it back and
  assert one `timeseries` cell bound to `federation.query`.
- **Rule / Insights** — the run + insights paths mount and complete honestly (no federation sidecar is
  spawned in this env — same as the Query-workbench gateway test — so the buildings query returns no
  rows here; we assert the paths run, never a fabricated result).

```
✓ src/features/admin/setup/DatasourceWizard.gateway.test.tsx (1 test)     # new
✓ src/features/admin/setup/AgentWizard.gateway.test.tsx (1 test)          # sibling unaffected
✓ src/features/admin/setup/IngestWizard.gateway.test.tsx (1 test)         # sibling unaffected
✓ src/features/rules + src/components/codeeditor (10 tests)               # CodeEditor change safe
```

`npx tsc --noEmit` clean; `eslint` clean on new files. Cap-deny is exercised per-step by gating
(`hasCap` on `datasource.list` / `dashboard.save` / `rules.run` / `insight.list`); a fresh `nextWs()`
per test isolates the shared node (workspace-isolation). The reused engines carry their own deny +
isolation coverage in their existing gateway tests.
