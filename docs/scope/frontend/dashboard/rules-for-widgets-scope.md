# Frontend scope — rules power widgets: any panel view renders a rule's rows, and the cage helps a rule return chart-shaped data

Status: scope (the ask). Promotes to `doc-site/content/public/dashboard/dashboard.md` once shipped.
Child of [`rules-as-source-scope.md`](rules-as-source-scope.md) (the **picker** half, shipped
2026-07-05) — this scope is the **render** half that doc marks blocked, plus the authoring
ergonomics that make it worth using. The blocker is precisely diagnosed in
[`../../../debugging/frontend/rules-as-source-render-path-empty.md`](../../../debugging/frontend/rules-as-source-render-path-empty.md).

A saved rule is already pickable as a panel source (`{tool:"rules.run", args:{rule_id}}`, Rules
group, typed params — shipped). But a panel bound to one renders **zero rows** for every view
(chart, table, stat, template): the host's `viz.query` recursive dispatch of `rules.run` fails
silently, and even when it succeeds the `RuleOutput` envelope (`{kind:"scalar"|"grid", …}`) is
never unwrapped into rows. We want the obvious thing to be true: **a rule is the most general
query — anything the cage can compute becomes a chart, a table, a stat tile, or a render
template**, because a rule has the whole data-stdlib (query/`source`, polars `Frame`, stats)
behind it. And we want returning chart-shaped data from a rule to be one line, not a convention
the author has to reverse-engineer — a small **chart-return helper family** in the cage.

## Goals

- **Close the render gap.** A panel/widget bound to `{tool:"rules.run"}` renders the rule's rows
  through the standard read path (`viz.query` → `usePanelData`) — for **every** view, with **zero
  view-side change** (the views are already source-agnostic; the gap is host-side).
- **Both `RuleOutput` kinds render.** A rule returning an array of row maps
  (`{kind:"scalar", value:[…]}`) and a rule returning a Grid/Frame
  (`{kind:"grid", columns, rows}`) both become frame rows — grid via the existing columnar path
  `federation.query` already uses.
- **Chart-return helpers in the cage** so "make this rule chartable" is one call, not folklore:
  normalize a time column, pivot long→wide for multi-series, and hand back rows the frame
  builder types correctly. The helpers live where the data-stdlib lives — one cage, every caller
  (rules workbench, `rules.eval`, the flows `rhai` node).
- **Panel-driven runs are read-only by default.** A dashboard auto-refreshing every 30 s must not
  spam the Inbox/Outbox with the rule's `alert()` routing on every repaint (see Risks).
- **The wizard gets it for free.** The new-panel wizard's Source step uses the shipped source
  picker (`useSourcePicker`), so the Rules group appears there with no wizard change — the E2E
  test proves the full loop: pick a rule → preview shows real rows → save → dashboard renders.

## Non-goals

- **No new host chokepoint, no rules special-case in `viz.query`.** The fix makes the *generic*
  recursive dispatch and the *generic* row-unwrapping correct; `rules.run` stays an opaque tool
  id to the viz plane (CLAUDE §10 held exactly as the parent scope holds it).
- **No rule authoring in the panel editor.** Authoring stays in the rules workbench/Playground;
  the panel consumes a saved rule by id (unchanged from the parent scope).
- **No live/streaming rules.** `rules.run` is one bounded run per refresh. A push-updating rule
  source (chains / `series.watch`-style) is future work — parent scope open question 3.
- **No findings-as-rows.** A rule's `emit`/`alert` findings are the **insights/inbox** plane's
  food, not chart rows (`../../insights/insights-package-scope.md` owns triage rendering). A
  panel renders the rule's **`output`** only. Surfacing `findings`/`log` in the Data Studio
  *inspector* for debugging is an open question, not a goal.
- **No static "is this rule chartable" classification.** Inherited stance from the parent scope:
  every saved rule is offered; a non-record output renders honestly empty. Save-time tagging
  stays that scope's open question 2.

## Intent / approach

Three slices, strictly ordered — the first is the unblock, the rest make it good.

### Slice 1 — the host render path (fix the two documented layers)

The debugging entry gives the exact fix list; this scope adopts it as the plan of record:

1. **Layer 1 — recursive dispatch.** Trace why `call_tool_at_depth("rules.run", …)` inside
   `viz_query` (`host/src/viz/query.rs::dispatch_target`) returns `Err` where the direct
   `rules_run` channel succeeds — prime suspects: the injected `ts`
   (`args.entry("ts").or_insert(json!(now))`) interacting with `resolve_rule_model`/
   `idem_prefix`, or a depth/routing difference. Instrument with a `tracing` span, fix the real
   cause; **do not** paper over with a rules branch in the dispatcher.
2. **Layer 2 — envelope unwrap.** Teach `host/src/viz/frame.rs::result_to_rows` the `RuleOutput`
   shape, generically-by-shape (not by tool id): an object with `{kind:"scalar", value:<array>}`
   → the value array; `{kind:"grid", columns, rows}` → the **existing** columnar zip path (it
   already handles `federation.query`'s identical shape — the grid arm may reduce to routing
   `output` into it). A full `RunResult` (`{output, findings, log, ms}`) unwraps `output` first.
   Mirror in the client `useSource.ts::toRows` per the existing lock-step comment.
3. **Un-skip the waiting regression test** — `templateView.gateway.test.tsx`'s
   `renders real rows from a RULES source` is already written and `it.skip`-ed with a
   fails-before note; it is the definition of done for this slice, plus a host-side integration
   test (`viz_query` over a seeded rule, both envelope kinds).

### Slice 2 — read-only panel runs (`rules.run {route:false}`)

Today every `rules.run` routes `alert()` findings to the Inbox + Outbox (`route_alerts` in
`host/src/rules/run.rs`). Correct for the workbench and for flows; wrong for a chart that
repaints on a 30 s auto-refresh — the alert id is stamped with `now`, so every refresh is a
**new** inbox item and a **new** must-deliver outbox entry. Fix: `rules.run`/`rules.eval` gain an
optional **`route: false`** argument (default `true` — existing behavior unchanged) that skips
`route_alerts`; findings still return in the result (honest, visible), they just don't fan out.
The **picker entry** sets `args.route = false` on the source it emits — the host composes args
(exactly like the shipped params form); `viz.query` never learns the flag exists. Rejected:
suppressing routing inside `viz.query` when the tool is `rules.run` (a core branch on a tool id —
the CLAUDE §10 leak), and deduping alerts by rule id (silently swallows legitimate re-alerts from
scheduled flow runs).

### Slice 3 — chart-return helpers in the cage (`verbs/chart.rs`, extends the data-stdlib)

The convention already exists (last expression = array of row maps; a time column makes it a
time-series) — but authors shouldn't have to know that SQLite returns ISO strings, series return
epoch, and the frame builder only types a `Time` column the resolver names. One small verb
family, sibling to `verbs/stats.rs`, pure compute, zero authority (data-stdlib doctrine):

| Helper | Does |
|---|---|
| `timeseries(rows, "ts")` | normalizes the named column across the shapes sources actually return (ISO-8601 string \| epoch-secs \| epoch-ms → canonical epoch-ms) and renames it `time`, so the frame builder tags the x-axis without guessing. Returns rows sorted by `time`. |
| `timeseries(rows, "ts", ["v1","v2"])` | same, plus keeps only `time` + the named value columns (shape trimming for the chart). |
| `wide(rows, "ts", "series", "value")` | long→wide pivot: one row per timestamp, one numeric column per distinct `series` value — the multi-line-chart shape. (Thin wrapper over the polars pivot the Frame already has; exists so the author doesn't need to learn `frame()` for the one daily task.) |
| `category(rows, "name", "value")` | the bar/pie shape: validates + trims to one label column + one numeric column. |

Each helper returns plain rows — `timeseries(query(…).records(), "ts")` as a rule's last line is
a complete chart-ready rule. `f.records()` and raw rows keep working; the helpers are sugar +
normalization, not a new required layer. Rejected: a `chart(#{…})` declarative return envelope
(a second output vocabulary beside `RuleOutput` that every consumer must learn); doing the
normalization host-side in `result_to_rows` (guessing which column is time from data is exactly
what `viz::frame` refuses to do — the author names it, the helper canonicalizes it).

## How it fits the core

- **Tenancy / isolation:** unchanged walls, re-proven on this path: the recursive dispatch runs
  under the **viewer's** principal, workspace-first — a rule saved in `acme` yields nothing for a
  `beta` viewer (the store read and the caps check both refuse before the cage runs). The helpers
  add zero data access (pure compute over rows already in the run).
- **Capabilities:** no new grants. The path is gated twice as today: `mcp:viz.query:call` for the
  panel resolve, `mcp:rules.run:call` re-checked inside the recursive dispatch under the caller's
  own authority (caller ∩ grant, no widening). Deny renders an honestly empty panel with the
  status bar's why-empty, not a crash. `route:false` needs no cap — it *reduces* effect.
- **Placement:** either — host crates + cage library, symmetric by construction. No `if cloud`.
- **MCP surface (§6.1):** **no new verbs.** The feature is: an existing read verb (`rules.run`)
  becomes consumable by the existing resolve verb (`viz.query`), plus one additive optional
  argument (`route`) on `rules.run`/`rules.eval`. CRUD/list/feed/batch all N/A — reads are
  bounded single runs; the live feed is an explicit non-goal.
- **Data (SurrealDB):** no new tables; the `SavedRule` record is read as today. State vs motion
  untouched.
- **Bus (Zenoh):** untouched. (`route:false` *skips* an outbox write; it adds no bus use.)
- **Sync / authority:** N/A — no new durable state.
- **Secrets:** none.
- **No mocks:** every test below runs against the real spawned gateway with real seeded rules —
  the parent feature's exact miss (a picker test standing in for a render test) is the lesson
  this plan encodes.
- **One responsibility per file:** the fix touches existing single-purpose files
  (`viz/query.rs`, `viz/frame.rs`, `rules/run.rs`); the helpers are one new verb-family file
  (`lb-rules/src/verbs/chart.rs`); never a `utils`.
- **SDK/WIT impact:** none — internal crates + one additive optional MCP arg.
- **Skill doc:** yes — two touched. `docs/skills/rules/SKILL.md` gains a **"returning chart
  data"** chapter (the helper family + the rows-with-a-time-column convention, grounded in a live
  run); `docs/skills/panels/SKILL.md` gains the "bind a panel to a saved rule" recipe once the
  render path is green. The implementing session owns both.

## Example flow

1. In the rules workbench, an author saves `energy-intensity`:
   ```rhai
   let rows = query("demo-buildings",
     "SELECT s.name AS building, substr(pr.time,1,10) AS ts,
             ROUND(SUM(pr.value),0) AS kwh
      FROM point_reading pr
      JOIN point p ON p.id = pr.point_id
      JOIN meter m ON m.id = p.meter_id
      JOIN site  s ON s.id = m.site_id
      WHERE p.name = 'Energy kWh'
      GROUP BY s.name, ts").records();
   wide(rows, "ts", "building", "kwh")     // one time column + one numeric column per building
   ```
2. In the new-panel wizard (or Data Studio), Source step → Rules group → `energy-intensity`.
   The picker emits `{tool:"rules.run", args:{rule_id:"energy-intensity", route:false}}`.
3. `viz.query` resolves the source under the viewer's caps, the recursive `rules.run` returns
   `{output:{kind:"scalar", value:[…]}}`, `result_to_rows` unwraps it, the frame builder tags
   `time`, and the preview draws one line per building — before the panel is even saved.
4. The panel is saved; the dashboard auto-refreshes every 30 s. The rule re-runs each time
   (bounded by `RuleLimits`); had the author left an `alert()` in the body, the finding returns
   in the result but routes nowhere (`route:false`) — the Inbox stays quiet.
5. The same rule, run by a scheduled flow (`rules.eval`, default `route:true`), still alerts —
   one rule, two consumption modes, no duplication.

## Testing plan

Per [`../../testing/testing-scope.md`](../../testing/testing-scope.md) — real store, real
gateway, seeded rules, no mocks:

- **The waiting regression test un-skips** (definition of done for slice 1):
  `ui/src/features/dashboard/views/templateView.gateway.test.tsx` `renders real rows from a
  RULES source (rules.run)` — green with **no template-side change**.
- **Host integration (Rust):** seed a saved rule per `RuleOutput` kind (scalar-array and grid);
  `viz_query` over `{tool:"rules.run"}` returns the rule's N rows for both; a full `RunResult`
  never collapses to one blob row (the Layer 2 regression pinned).
- **Unit:** `result_to_rows` on `{kind:"scalar", value:[…]}`, `{kind:"grid", columns, rows}`,
  a whole `RunResult`, and a scalar non-array output (→ honest single value row); the client
  `useSource.ts::toRows` mirror cases in lock-step.
- **Capability-deny (mandatory):** a viewer with `mcp:viz.query:call` but **without**
  `mcp:rules.run:call` gets an empty/denied resolve for that source (and the status bar says
  why) — not a 500, not another source's data.
- **Workspace-isolation (mandatory):** rule saved in `acme`; a `beta` principal's `viz.query`
  over it yields no rows; `beta`'s picker never offered it (re-asserting the parent's shipped
  test at the render layer).
- **Route flag:** a rule body with `alert()` run via `rules.run {route:false}` → findings in the
  result, **zero** new inbox items and **zero** outbox entries (count before/after in the real
  store); the same body with default routing still lands both (existing behavior pinned).
- **Helpers (unit, table-driven):** `timeseries` across the three timestamp shapes
  (ISO / epoch-secs / epoch-ms), unsorted input, missing column (clear author error);
  `wide` on a long fixture (row count = distinct timestamps, column count = distinct series + 1);
  `category` trims and validates; all pure/deterministic (run-twice-identical, data-stdlib
  contract).
- **E2E (gateway):** the wizard loop — seed rule → Source step offers it → preview renders real
  rows → save → `WidgetView` renders the chart on the dashboard route.
- Anything that breaks logs under `docs/debugging/frontend/` (render path) or
  `docs/debugging/rules/` (cage helpers), with a regression test.

## Risks & hard problems

- **Layer 1's root cause is undiagnosed.** The `Err` in the recursive dispatch is located but not
  explained; if it turns out to be structural (e.g. the idempotency model resolution genuinely
  can't run at dispatch depth > 0), the fix could grow. First task of the build: the tracing
  span + a minimal failing Rust test, *before* writing any fix.
- **Alert spam is a footgun with real blast radius** (Inbox noise + must-deliver outbox traffic
  on every dashboard repaint, per viewer). That's why `route:false` is slice 2, not a follow-up —
  the render path must not ship without it.
- **A rule per panel per refresh is real compute.** A heavy rule (big `frame()`, slow federation
  query) on a 5 s auto-refresh multiplies. The existing governors (`RuleLimits`, `viz.query`
  timeout) bound each run; they do not bound *frequency*. Acceptable for v1 (same exposure as a
  heavy SQL source today) — but the skill doc must say "heavy rules + fast refresh don't mix",
  and result caching is named below as the future lever.
- **Shape guessing creep.** The temptation in Layer 2 is to make `result_to_rows` "smart" about
  ever more envelopes. The line to hold: unwrap *documented shapes by structure*
  (`kind`-discriminated), never heuristics over arbitrary objects — misshaped output renders
  empty, honestly.

## Open questions

1. **Params from dashboard variables.** A rule param filled from `$site`/`$__from` (the shipped
   variable interpolation) would make one rule serve a template-group of pages. Does the shipped
   `args` interpolation already reach `args.params.*` (nothing rule-specific needed), or does the
   params form need a "bind to variable" affordance? Verify first — lean: it already works,
   document it in the skill.
2. **Findings/log in the Data Studio inspector.** When the source is a rule, the inspector could
   show the run's `log` and `findings` next to the rows — the debugging affordance for "why is my
   chart empty". Additive UI, no host change (the data is already in the result). Lean: yes, as a
   fast-follow within Data Studio's status-bar/inspector surface.
3. **Result caching / shared runs.** N viewers of one dashboard each trigger their own rule run.
   A short-TTL host-side cache keyed by (ws, rule_id, params, route) would collapse them — but
   caching under *whose* caps is subtle (two viewers with different grants must not share a
   result). Defer until measured; note it in the perf ledger.
4. **`route` naming + reach.** `route:false` vs `dry_run` vs `no_alerts` — and should the flows
   `rule` node expose the same knob? Lean: `route` (it routes findings; the run itself is real,
   so `dry_run` misleads), exposed anywhere `rules.run`/`rules.eval` args are composed.

## Related

- [`rules-as-source-scope.md`](rules-as-source-scope.md) — the parent: picker + params, shipped;
  this scope closes its blocked render half.
- [`../../../debugging/frontend/rules-as-source-render-path-empty.md`](../../../debugging/frontend/rules-as-source-render-path-empty.md)
  — the two-layer diagnosis slice 1 executes.
- [`../../rules/data-stdlib-scope.md`](../../rules/data-stdlib-scope.md) — the cage library the
  chart helpers extend (`verbs/` doctrine, zero-authority pure compute).
- [`../../rules/rules-engine-scope.md`](../../rules/rules-engine-scope.md) — `rules.run`/
  `rules.eval`, `RuleOutput`, governors, alert routing.
- [`render-template-inprocess-scope.md`](render-template-inprocess-scope.md) — the template view
  that surfaced the gap; renders rule rows with zero change once slice 1 lands.
- [`data-studio-ux-scope.md`](data-studio-ux-scope.md) — the status bar / inspector where open
  question 2 would live.
- [`../../insights/insights-package-scope.md`](../../insights/insights-package-scope.md) — where
  findings-as-triage rendering lives (the non-goal boundary).
- `docs/skills/rules/SKILL.md`, `docs/skills/panels/SKILL.md` — the skills the build extends.
- README §3 rules 5 (capability-first), 6 (workspace wall), 9 (no mocks), 10 (core knows no
  extension — the dispatcher stays generic).
