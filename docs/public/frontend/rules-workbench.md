# Rules workbench (public)

Status: **shipped** (Phases 1–3). Scope: `../../scope/frontend/rules-workbench-scope.md` · Session:
`../../sessions/frontend/rules-workbench-session.md`.

A first-party shell surface in three cap-gated pages — **Playground** (write/run/save a Rhai rule),
**chain canvas** (a React Flow DAG over saved rules, coloured as steps settle), and **datasources admin**
(register/test external sources) — driving the **already-shipped** `rules.*`/`chains.*`/`datasource.*`
host verbs over the gateway, mirroring the dashboard surface verb-for-verb. No host work was added; the
slice is gateway routes + UI api clients + the React surface. The federation extension stays headless;
the datasources page is trusted first-party shell code, not extension-contributed UI.

## What exists

### Playground (`features/rules/`)
- **Editor:** a CodeMirror editor (`@uiw/react-codemirror` + `lang-javascript`) for the Rhai body, with
  a dirty indicator; a run uses the live buffer (ad-hoc `body`), a save persists it.
- **Run:** `rules.run {body|rule_id, params}` renders the typed `RuleOutput` three ways by `kind` —
  **scalar** (`ScalarCard`), **grid** (`GridTable`, bounded rows + "showing N of M"), **findings**
  (`FindingsList`, level-coloured, alert-marked) — plus a `LogPanel` and a `BudgetBadge` (ms + ai
  calls/tokens).
- **CRUD rail:** `rules.list`/`get`/`save`/`delete` — the saved-rule roster opens/saves/deletes records.
- **Honest cage/deny states:** a denied source, an AI-budget abort, a cage error (`eval`/`import`/loop/
  oversize), and "AI not configured" each render as themselves — `BadInput` shown verbatim (author
  feedback), `Denied` a generic "not permitted". Never a fake result, never a generic toast.

### Guided authoring (`features/rules/panel/`, `components/codeeditor/`, `components/schema/`, `lib/schema/`)

The Playground editor is a **guided, explorable authoring surface** (scope:
`../../scope/frontend/rules-editor-ux-scope.md`). Alongside the editor sits a tabbed authoring panel —
**Functions | Examples | Data** — so a user who has never written a Rhai rule can discover the vocabulary
and one-click it in:
- **Function palette** — a searchable, categorized list of the engine's **registered** verbs (Data · Grid ·
  Timeseries · AI · Output), each with its signature + one-line summary, **click-to-insert** at the cursor.
  The catalog is a **static typed mirror** of `rust/crates/rules/src/verbs/*` (the registered set is
  compile-time known — no host "list functions" verb is invented); one data file per verb family.
- **Examples** — ready-to-run example rules; one click loads the body into the buffer (a **dirty-confirm**
  guards unsaved edits). Bodies reuse the proven gateway-test bodies so they run green.
- **Data explorer** — the registered external datasources (`datasource.list` → `source("name")`), the local
  store schema (`store.schema` → a table/column name), and the discoverable series (`series.list` →
  `history("series", name, "24h")`), each **click-to-insert**. Each section renders an **honest** state —
  loading skeleton, deny (never a fabricated roster), empty-that-teaches, or ready. The DSN never renders
  (only the redacted ref `datasource.list` returns). *Honest gap:* deep per-external-datasource table
  introspection needs a host verb that doesn't exist — a **named follow-up**, not a silent omission.

**Reusable pieces** (extracted, consumed by more than one surface):
- `lib/schema/` — the `store.schema` reader (`readSchema`/`Schema`/…), extracted from the dashboard-named
  `lib/dashboard/sql.api.ts` so **both** the dashboard SQL builder **and** the rules data explorer consume
  one module (the dashboard SQL builder keeps its typed-query dropdowns; the explorer uses a click tree).
- `components/codeeditor/` — a controlled `CodeEditor` with an `insertSnippet` ref handle (a real CodeMirror
  transaction): the one click-to-insert primitive every palette/explorer panel uses.
- `components/schema/SchemaBrowser` — a reusable collapsible table→column tree with click-to-pick.

This slice added **no host work, no new MCP verbs, no new caps** — it is reusable UI components + the editor
surface over the shipped `rules.*`/`datasource.*`/`store.schema`/`series.*` verbs.

### Chain canvas (`features/chains/`)
- A React Flow (`@xyflow/react` v12) DAG: **nodes = steps** (each names a saved rule), **edges =
  `needs`**, mapping 1:1 to `chain.steps[].needs`. Save via `chains.save` (host-validated up front — a
  cyclic/invalid edge renders the host's error **inline**, no crash).
- **Run + settle colouring:** `chains.run` → `{run_id}`; the canvas **bounded-polls** `chains.runs.get`
  (poll while non-terminal, stop on terminal, a ceiling on attempts — never an unbounded interval) to
  colour each node `pending → running → ok | err | skipped`, the Halt-pruned subtree greyed, with a
  status banner (`success`/`partialFailure`/`failed`). A late open rebuilds the colours from the
  snapshot. (`chains.watch` SSE is the named follow-up; this ships the durable-snapshot poll.)

### Datasources admin (`features/datasources/`)
- `datasource.list` (kind + endpoint + a **redacted** secret ref — never a DSN), `datasource.add`
  (the form is the only place a DSN exists client-side; it is written host-side to `lb_secrets` and never
  read back), `datasource.remove`, and `datasource.test` (a real green/red connectivity probe — an
  honest red when no sidecar is available, never a fabricated green). The Add form shows the implied
  `net:tls:host:port:connect` + `secret:federation/{name}:get` grants.

### Gateway + auth
- **Routes:** `routes/{rules,chains,datasources}.rs` — `POST /rules/run`, `GET|POST /rules`,
  `GET|DELETE /rules/{id}`; `GET|POST /chains`, `GET|DELETE /chains/{id}`, `POST /chains/{id}/run`,
  `GET /chains/{id}/runs/{run_id}`; `GET|POST /datasources`, `DELETE /datasources/{name}`,
  `POST /datasources/{name}/test`. Each re-checks the cap server-side via `lb_host::call_tool` (which
  also wires the `DisabledModel` AI seam and the `OsLauncher` sidecar) and derives workspace + principal
  from the **token**, never the body. `ToolError`→HTTP: `Denied`→403 opaque, `BadInput`→400 verbatim,
  `NotFound`→404.
- **Cap-gated nav:** the Rules/Chains/Datasources surfaces show on `mcp:rules.run`/`chains.get`/
  `datasource.list` respectively (display convenience); the gateway re-checks every verb.

## Isolation & security

Every `rule:{ws}:{id}`, `chain:{ws}:{id}`, `datasource:{ws}:{name}` is workspace-namespaced — a ws-B
session sees/runs none of ws-A's. The UI is a caller of existing caps (no new caps, no new tables, no
`localStorage` durable state, no `if cloud`). Inside a run, every data verb still hits `caps::check`
under `caller ∩ grant` — the page cannot widen it.

## Tests

Real in-process gateway, seeded via the real write path (no mocks, no `*.fake.ts`):
- **Rust gateway:** `rules_routes_test` (13), `chains_routes_test` (5), `datasources_routes_test` (5) —
  CRUD round-trips (roster contains the saved record), a deny-test per verb, two-session workspace
  isolation, the cage/deny honesty cases (cage→400 verbatim, AI-not-configured→400, cyclic DAG→400),
  the run→`runs.get` settle snapshot, and the DSN-redaction assertion.
- **UI Vitest:** `RulesView`/`ChainsView`/`DatasourcesAdmin` `.gateway.test.tsx` (6/4/5) — the three
  output kinds, CRUD, honest error states, chain settle-colouring, the datasource probe + DSN redaction.
- **UI Vitest (guided authoring):** `panel/AuthoringPanel.gateway.test.tsx` (6) — the palette renders the
  real categories + click-to-insert; search filters; an example loads + runs green; the dirty-confirm
  guard; the data explorer lists a real datasource + schema + series (no DSN) + a denied section's honest
  deny. The extracted `lib/schema` reader is proven in **both** consumers (the dashboard
  `sqlSource.gateway.test.tsx` stays green over it).

## Notes

Building the rail surfaces surfaced (and this slice fixed) a shipped host bug: `rules.list`/`chains.list`
decoded the `lb_store::scan` envelope row directly and silently dropped every record — fixed to unwrap
the `{data}` envelope (mirroring `scan_dashboards`), with regression tests. See
`../../debugging/host/rules-chains-list-drops-every-row-envelope.md`.
