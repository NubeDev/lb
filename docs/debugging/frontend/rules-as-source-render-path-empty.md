# A `template`/chart/table cell bound to `{tool:"rules.run"}` renders ZERO rows (the rules-as-source render path is empty)

- Area: frontend / host
- Status: **open** (surfaced during the render-template-inprocess scope; out of that scope's "no
  pipeline change" boundary)
- First seen: 2026-07-05
- Session: [`../../sessions/frontend/dashboard/render-template-inprocess-session.md`](../../sessions/frontend/dashboard/render-template-inprocess-session.md)
- Regression test: `ui/src/features/dashboard/views/templateView.gateway.test.tsx` — the
  `renders real rows from a RULES source (rules.run)` case is `it.skip` with a precise note (fails-
  before: `rows.length === 0`; will pass once the host gap is fixed, with NO template-side change).

## Symptom

A `template` cell (or a `chart`/`table`/`stat`/any view) bound to a Rules source —
`sources:[{tool:"rules.run", args:{rule_id:"hourly"}}]` — renders **zero rows** through the dashboard
read path (`viz.query` → `usePanelData`). The template's `{{rows.length}}` resolves to `0`; a chart
shows nothing. The source **does** return rows on the **direct** `rules.run` route.

## Root cause (two layers, both in the host pipeline — NOT in any view)

Layer 1 — **the recursive dispatch returns empty.** `viz.query` resolves a panel's `sources[]` by
re-entering `call_tool_at_depth("rules.run", …)` for each target (`viz/query.rs::dispatch_target`).
Against a real spawned gateway, with explicit caps `mcp:rules.run:call` + `mcp:viz.query:call` +
`store:rule:read/write`, that recursive call yields an EMPTY row set — `dispatch_target`'s `Err(_) =>
Vec::new()` arm fires. The **direct** `rules_run` route (the `rules_run` invoke channel the rules
workbench uses) returns the rows. So the gap is in the `viz.query` → `call_tool_at_depth("rules.run")`
path specifically, not in `rules.run` itself.

Verified live (the smoking pair, against a real node):
```
runRule({ruleId:"hourly"})  →  output = {kind:"scalar", value:[{h:0,v:10},{h:1,v:20},{h:2,v:30}]}   ✓
template over {tool:"rules.run",args:{rule_id:"hourly"}}  →  rows.length = 0                          ✗
```
(rule body: `let rows = [#{ h: 0, v: 10 }, #{ h: 1, v: 20 }, #{ h: 2, v: 30 }]; rows`.)

Layer 2 — **the `RuleOutput` envelope is not unwrapped.** Even if Layer 1 were fixed,
`viz::frame::result_to_rows` would still mis-shape a `rules.run` result. `RunResult` serializes as
`{output, findings, log, ms, ai}` where `output` is a `RuleOutput` envelope `{kind:"scalar", value:[…]}`
or `{kind:"grid", columns:[…], rows:[…]}` (`rules/src/runtime.rs::RuleOutput::Serialize`).
`result_to_rows` checks `ROW_KEYS = [samples, items, rows, templates, dashboards, reminders]` — none
match `output` (and `output` is an object, not a bare array), so a full `RunResult` would collapse to
ONE JSON-blob row (the whole RunResult), not the N rows the rule returned. (A one-line
`ROW_KEYS += "output"` was tried and reverted: `output` is an envelope object, never a bare array, so
the addition is a no-op — the real unwrap needs to understand the `{kind, value|columns+rows}` shape.)

## Why it wasn't caught

The rules-as-source feature shipped its **picker** parity against the real gateway
(`rulesSource.gateway.test.tsx` exercises `loadSourcePicker` — the Rules group appears, workspace-
isolated, deny-tolerant). That test never drives a VIEW bound to the rule through `viz.query`, so the
render path was untested. `runRule` (the rules workbench path) hides Layer 1 because it uses the
dedicated `rules_run` channel, not the `viz.query` recursive dispatch.

## Impact

The scope's headline ("Rules (and every other source) work for free") is **not yet true for the render
path**: a dashboard or channel view bound to a rules source renders empty, for ANY view (template,
chart, table, stat). The PICKER and the rules workbench are unaffected. The new in-process
`TemplateView` is source-agnostic and correct — it renders whatever `usePanelData` resolves (proven by
the series/SQL gateway test rendering 3 real rows) — so it will render rule rows the moment this host
gap is fixed, with **no template-side change**.

## Fix (for the host owner — separate scope)

1. **Layer 1:** trace why `call_tool_at_depth("rules.run", …)` from inside `viz_query` returns `Err`
   (so `dispatch_target` falls to its empty-rows arm). Likely candidates: the `ts`/`now` the dispatch
   injects (`dispatch_target` does `args.entry("ts").or_insert(json!(now))`) interacting with
   `resolve_rule_model` / `idem_prefix`; OR a depth/routing difference between the recursive dispatch
   and the direct `rules_run` channel. A `tracing` span around the recursive call will surface the
   `Err` variant + message.
2. **Layer 2:** teach `result_to_rows` to unwrap the `RuleOutput` envelope — when the object has
   `{kind:"scalar", value: <array>}` return the value array; when `{kind:"grid", rows: <array>}` follow
   the existing columnar path. (And mirror it in the client `useSource.ts::toRows` ROW_KEYS list, per
   the existing lock-step comment.)
3. Add a host integration test: a panel bound to `{tool:"rules.run"}` renders the rule's rows through
   `viz_query` end to end (the gateway test's `it.skip` case un-skips once green).

## Lesson

A "shipped" picker is not a shipped **render** path. The rules-as-source feature shipped the source-
picker half (selecting `rules.run` produces `{tool, args}`); the data-flow half (a view binding that
source and rendering its rows through the panel-data hook) was never driven against the real gateway,
so a silent empty-rows gap lived in the recursive dispatch. When a feature spans "select a source" AND
"render a view over it", the render half needs its own real-gateway test — the picker test doesn't
cover it.
