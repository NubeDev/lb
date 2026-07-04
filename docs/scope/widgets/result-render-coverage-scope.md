# Widgets scope — Slice C: result-render coverage (every tool declares its output widget)

Status: scope (the ask). Promotes to `public/frontend/dashboard.md` (a "Result-render coverage"
section beside Slice B's) once shipped. Topic: `widgets` (the umbrella program).

**Slice C of the system-wide widget program**
([`widget-platform-scope.md`](widget-platform-scope.md) § "Slice C — Result-render coverage"). The
umbrella's Slice C section is a 6-line sketch; this doc is the build-ready scope. It closes **G1**
(schema coverage is thin on the OUTPUT half): today only **one** host tool (`reminder.list`)
declares a `descriptor.result` render envelope; `federation.query`, `agent.invoke`, and
`query.*` still render via hardcoded client branches (rich-responses follow-up #5). "Every tool/API
is a widget with a JSON schema in **and** out" is not yet true. Slice C makes it true for the
**tabular** tools (`federation.query`, `query.run`) by giving each a `result = table` envelope —
so the channel **can** render them descriptor-driven, the AI **discovers** the render via
`tools.catalog`, and Slice B's `dashboard.pin` **can** pin them with ZERO tool-specific code in the
pin path (the headline).

## What this slice IS — backend config, no new verb

Each new envelope is a **`result:` field on the tool's `ToolDescriptor`** — pure backend config
beside the verb (FILE-LAYOUT; one `descriptor()` per verb file, collected by `host_descriptors`).
No new MCP verb, no new cap, no new table, no SDK/WIT change. The generic mechanisms that *consume*
the envelope are UNCHANGED:

- **`tools.catalog`** already serves `descriptor.result` (it shipped with the field). The catalog's
  per-tool `authorize_tool` gate already decides visibility — naming the descriptor `federation.query`
  means the run's existing `mcp:federation.query:call` gate decides its visibility with zero
  special-casing (the same model `reminder.list` established). No new cap.
- **The channel palette's generic `tool.result` path** (`CommandPalette.tsx` line ~228: collect args
  → interpolate into `source.args` → `onPostRich(encodeRichResult(...))`) already posts ANY
  descriptor's `result` envelope tool-agnostically. A descriptor that gains a `result` is rendered
  by the channel through that path with NO UI change.
- **`ResponseView` + `WidgetView`** already mount any `rich_result` envelope through the one shipped
  dispatcher. A `result = table` envelope renders as a table (the same `TablePanel`/`ResponseTable`
  the reminder widget uses), data flowing through the gated bridge on render.
- **`dashboard.pin`** (Slice B) already consumes ANY envelope generically — `mint_cell_from_envelope`
  treats `source.tool` as opaque data, no branch on the id (rule 10). A `federation.query` envelope
  mints a `pin-federation-query` cell, persisted through the Slice A validation chain, that re-runs
  `federation.query` under the viewer's grant at render. The HEADLINE proof.

## The tool → view mapping (and the tools this slice DELIBERATELY skips)

Slice C covers the **tabular** host tools — tools whose answer is a `{columns, rows}` result set
that normalizes to rows through `viz::frame::result_to_rows` (which already zips columnar arrays
into named row objects, exactly for `federation.query`). For each, the descriptor gains a `table`
envelope whose `source.tool` names the tool itself (the channel/pinned cell re-runs it at render
under the viewer's grant) and whose `source.args` is the empty template the palette interpolates the
collected form fields into (same shape `reminder.list`'s `list_render()` established).

| Tool | View | Why |
|---|---|---|
| **`federation.query`** | `table` | Returns `{columns, rows}` — the columnar shape `result_to_rows` is written for. A pinned cell re-runs the registered-source read under the viewer's grant (workspace-walled by the source namespace; the bridge leash re-checks `mcp:federation.query:call` per render). The **headline** of the slice. |
| **`query.run`** | `table` | Returns the SAME `{columns, rows}` shape (run.rs:97). Re-runs the saved/inline query at render. A pinned `query.run` cell is the durable home for "this query on a dashboard" (today only the channel `kind:"query"` worker captures a one-shot result). |
| ~~`agent.invoke`~~ | ~~its render~~ | **SKIPPED in Slice C — see "Why agent.invoke is deferred" below.** |

`query.save` (a write verb — its answer is the saved record, not a render) and `query.compile` (a
dry-run that returns SQL text, marginally useful as a `code` view) are out of scope as Named
Follow-ups; Slice C's headline is the tabular read path.

### Why `agent.invoke` is deferred (the long-term-right call)

The umbrella sketches "`agent.invoke` → its render" — but a defensible look at the agent's actual
render shows it is **not** a pin-able widget in the source-rerun sense, and Slice C should not ship
an envelope that implies it is. Specifically:

- **The agent's render is streaming + nondeterministic.** `agent.invoke` does not return a stable
  result set; it kicks off a durable run whose events (`agent_step`/`agent_result`) are rendered by
  `AgentCard` from the run feed. A `rich_result` with `source.tool = "agent.invoke"` would **re-run
  the agent on every render** of the pinned cell — a different answer every dashboard load, at model-
  call cost. That is semantically wrong for a dashboard widget, which is supposed to be a stable view
  of data.
- **The right path for agent results is Slice D, not Slice C.** Slice D (channel-origin authoring:
  response → widget → preview → `dashboard.pin`) is where an agent's one-shot ANSWER can be captured
  as a **`data`-backed** envelope (inline snapshot, not a source re-run) and pinned — the pinned
  widget shows the captured answer, not a live re-run. That respects both the agent's nondeterminism
  and the dashboard's stability contract. Slice C's source-rerun model is the wrong shape for that.
- **The shipped `kind:"agent"` palette route carries the streaming workflow.** It is NOT just a
  render branch — `postAgent` posts a structured channel Item that the host agent worker picks up,
  drives the run, and posts a durable `agent_result` back. A static `result` envelope cannot replace
  that workflow (a descriptor is a template, not a job enqueuer). So even if `agent.invoke` carried a
  `result` envelope, the palette's `kind:"agent"` branch would (correctly) still fire first.

**Rejected alternative — give `agent.invoke` a no-`source` `view:"markdown"` envelope.** Tempting
(the descriptor would advertise "my answer is markdown"), but the generic palette path does not CALL
the tool — it posts the descriptor's `result` template verbatim. With no `source`, the posted
envelope carries no agent answer at all (empty markdown). Useless to render, useless to pin. Slice D
solves this by snapshotting the actual answer into `data` at pin time; Slice C cannot.

This is the slice's only meaningful design call and it does NOT contradict the umbrella — the
umbrella's "agent.invoke → its render" is sketch-level; the long-term-right reading is that an
agent's render belongs to Slice D's snapshot model, not Slice C's source-rerun model. The umbrella's
Slice C section is reframed accordingly (recorded in "Open questions refreshed" below).

## Goals

- **`federation.query` and `query.run` each carry a `descriptor.result = table` envelope.** Each
  envelope is the shape `reminder/descriptor.rs::list_render()` established: `{ v:2, view:"table",
  source:{tool:<self>, args:{}}, tools:[<self>] }`. The `source.tool` names the tool itself (the
  re-runnable read); `source.args` is the empty template the palette interpolates into; `tools[]`
  is the declared bridge set (just the read tool itself for these — no row-control write verbs).
- **The headline: `federation.query`'s NEW `result` envelope is pin-able via `dashboard.pin`.** Pin
  the descriptor's `result` → a persisted `pin-federation-query` cell that reloads via
  `dashboard.get` and renders through the real `WidgetView`, with ZERO federation-specific code in
  the pin path (the mint function treats the tool id as opaque data — Slice B proven, re-asserted
  here for the new envelope).
- **The channel renders the new envelopes descriptor-driven.** A UI gateway test posts the
  descriptor's `result` envelope (interpolated with seeded args) as a `rich_result` Item and asserts
  it mounts through `ResponseView` → `WidgetView`. The descriptor-driven path is PROVEN to work for
  these tools — it is no longer true that only the hardcoded branches can render them.
- **The tools' existing gates decide their catalog visibility** (no new cap, no `if` in the catalog
  — the name IS the gate). A caller without `mcp:federation.query:call` doesn't see the command (so
  doesn't see the render either — the menu IS the permission model).

## Non-goals

- **No new MCP verb / cap / table / WIT.** Slice C is BACKEND CONFIG (descriptor `result` fields)
  + retiring the rendering half of rich-responses follow-up #5. The pin path is unchanged (Slice B);
  the catalog is unchanged; the bridge is unchanged; `viz::frame::result_to_rows` is unchanged.
- **No retiring the `kind:"query"` / `kind:"agent"` palette ROUTING branches.** Follow-up #5
  conflates two concerns: RENDERING (a tool's answer mounts as a widget — Slice C retires this half
  for the tabular tools) and ROUTING (which payload KIND the palette emits — `kind:"query"` for the
  async query-worker, `kind:"agent"` for the streaming run, `kind:"rich_result"` for the
  source-rerun model). The routing branches carry ASYNC/STREAMING workflow semantics a static
  descriptor cannot express; they stay. Slice C reframes follow-up #5 (see Open questions) — the
  rendering half is closed for `federation.query`/`query.run`; the routing half is intentional.
- **No `agent.invoke` envelope (deferred to Slice D).** See "Why agent.invoke is deferred" above.
- **No `query.save`/`query.compile` envelope.** `query.save` is a write verb; `query.compile`'s
  SQL-text answer is marginally useful as a `code` view. Named follow-ups, not this slice.
- **No option-key validation, per-widget version stamping, ext-key install-resolve** — Slice A
  follow-ups, deferred.
- **No `dashboard.pin` change.** Slice B already consumes any envelope generically. Do NOT touch
  `pin.rs` unless a Slice C envelope reveals a real mint gap (surface it then, don't silently widen).
- **No batch / live feed / job.** Slice C adds static config; it owns no record and emits no event.

## How it fits the core

- **MCP is the universal contract (rule 7).** The new envelopes are descriptor config served over
  the EXISTING `tools.catalog`; pinning goes through the EXISTING `dashboard.pin`; rendering goes
  through the EXISTING `rich_result` → `ResponseView` path. No new seam.
- **Core knows no extension / no tool special-case (rule 10).** The pin path and the bridge treat
  `federation.query` as opaque data (a `source.tool` string). The mint function (Slice B) already
  proven generic over the tool id; Slice C adds NO `match`/`if` on a tool id anywhere. The
  descriptors themselves name their own tool as `source.tool` — that's a tool naming ITSELF in its
  own descriptor (in its own file), not a core branch.
- **Capability-first (rule 5).** The catalog's per-tool `authorize_tool` (catalog.rs:51) already
  drops a tool the caller can't call — so a caller without `mcp:federation.query:call` doesn't see
  the command, doesn't see the render, doesn't get the envelope. No new cap; the verb's own call
  gate decides. The render-time re-check (the bridge leash `cellTools(cell) ∩ grant`, re-checked at
  the host per call) is unchanged — a pinned `federation.query` cell still re-checks
  `mcp:federation.query:call` on every render under the viewer's grant.
- **Workspace wall (rule 6).** A pinned `federation.query` cell's `source.tool` re-runs under the
  viewer's grant AT RENDER, and `federation_query` resolves the source alias in the viewer's
  workspace namespace (federation/query.rs:42) — a ws-B viewer cannot name a ws-A source. Tested
  with the mandatory two-session isolation case.
- **State vs motion (rule 3).** A pinned cell is state (`dashboard:{id}`); a channel response is
  motion+history. Slice C produces no new persistence — the envelope is config, the cell is the
  existing persisted form.
- **One datastore (rule 2).** No new persistence. The descriptor config lives in code (beside the
  verb); the pinned cell lives on the existing `dashboard:{id}` record.
- **Symmetric nodes (rule 1).** Pure node-local descriptor config; no cloud authority, no `if cloud`.
- **Skill doc.** Yes — extends `skills/dashboard-widgets/SKILL.md` with **the list of tools that
  declare a `result` render today** (`reminder.list` + the Slice C additions), grounded in a live
  `tools.catalog` run. The slice's implementing session writes/updates it.

## Example flow

1. **Discovery.** A member (or an AI agent) calls `tools.catalog`. The catalog returns the
   `federation.query` descriptor WITH its new `result = { view:"table", source:{tool:
   "federation.query"}, tools:["federation.query"] }` — the AI now sees "this tool's answer renders
   as a table over itself."
2. **Channel render (the new capability).** The member runs `/query` from the palette. The legacy
   `kind:"query"` branch fires (the async worker workflow — kept), and the channel renders the
   worker's `query_result` Item via the shipped `QueryCard` path. **OR**, if a future palette change
   routes through the descriptor-driven path: the palette posts the descriptor's `result` envelope
   (interpolating the collected `source`/`sql` into `source.args`) as a `rich_result` Item, and
   `ResponseView` mounts it through `WidgetView` — proven by Slice C's UI gateway test. Both paths
   work; the descriptor-driven path is NEWLY available.
3. **Pin (the HEADLINE).** A headless AI agent reads `federation.query`'s `result` from
   `tools.catalog` and pins it: `POST /mcp/call dashboard.pin { dashboard:"ops", envelope:
   <descriptor.result with source.args={source,sql}>, now }`. The host mints a `pin-federation-query`
   cell (generic over the tool id), runs the Slice A validation chain, persists. The next
   `dashboard.get` for "ops" returns the cell; `WidgetView` renders it; the bridge re-dispatches
   `federation.query` under the viewer's grant → rows. No federation-specific code in the pin path.
4. **Workspace isolation.** A ws-B viewer opening the "ops" dashboard sees the cell (it's on a
   dashboard record in ws-A's namespace — wait, no: dashboards are workspace-scoped, so ws-B never
   sees ws-A's "ops"). And even if a ws-B viewer COULD see the cell, the cell's `source.tool =
   federation.query` re-runs under the viewer's grant at render → `federation_query` resolves the
   source alias in ws-B's namespace → ws-B's source (or none). The wall is structural.

## Testing plan

Real gateway + real store, no fakes (rule 9). Mirror Slice B's test file
(`rust/crates/host/tests/widget_pin_test.rs`) + Slice A's (`widget_catalog_test.rs`) for structure.
Mandatory categories:

- **Per-tool descriptor unit tests (Rust).** Each new `result` envelope asserts it carries the right
  shape: `v:2`, `view:"table"`, `source.tool` names the tool itself, `source.args` is an object,
  `tools[]` includes the tool. Mirrors `reminder/descriptor.rs::list_descriptor_carries_the_interactive_table_render`.
- **Capability-visibility (the catalog gate).** `tools.catalog` for a principal WITHOUT
  `mcp:federation.query:call` does NOT include the `federation.query` descriptor (so no command, no
  render, no envelope leak). The paired happy path: a principal WITH the cap sees it WITH its `result`
  envelope. (Not a new cap-deny — asserting the EXISTING gate decides the envelope's visibility too.)
- **Workspace isolation (required).** A `federation.query`/`query.run` result rendered in ws-A leaks
  no ws-B rows: the cell's `source.tool` re-runs under the viewer's grant at render, and the source
  alias resolves in the viewer's workspace namespace. Asserted at the pin/persist layer (a ws-B pin
  mints a ws-B cell on a ws-B dashboard) AND at the render-resolve layer (the source alias is
  workspace-walled — federation/query.rs:42 already asserts this, Slice C re-asserts via the cell).
- **The HEADLINE (integration + UI).** Pin `federation.query`'s NEW `result` envelope via
  `dashboard.pin` → a persisted `pin-federation-query` cell that reloads via `dashboard.get` and
  carries the envelope's `view`/`source`/`tools` intact. ZERO federation-specific code in the pin
  path (the mint treats `source.tool` as opaque data — Slice B proven, re-asserted for the new
  envelope). The UI gateway test posts the descriptor's `result` (interpolated with seeded args)
  as a `rich_result` Item and asserts it mounts through `ResponseView`/`WidgetView` (a real table
  over a real source — seeded via the real write path, no fake).
- **No-regression.** Slice B's `widget_pin_test` (10/10) stays green — the new envelopes mint
  through the same generic path. Slice A's `widget_catalog_test` (8/8) stays green — the catalog
  serves the new envelopes through the same per-tool gate. `pnpm test` (unit) stays green. The
  retired-rendering-half claim is proven by the UI gateway test (the descriptor-driven path renders
  the new envelope identically to how a hand-built one would).

A UI unit test is NOT required for this slice — there is NO new UI component (the existing
`ResponseView`/`WidgetView`/`PinToDashboard` already consume any envelope). The UI gateway test is
the parity proof that the existing components render the new envelope.

## Risks & hard problems

- **The source-rerun model is the load-bearing claim.** A pinned `federation.query` cell RE-RUNS
  the query on every dashboard load — under the viewer's grant, against the cell's captured
  `source.args`. That is the right semantic for a dashboard widget (live data), but it means a
  pinned expensive query is expensive on every view. Mitigation: the existing `federation_query`
  row cap + the per-panel `viz_query` frame budget (`MAX_ROWS_PER_FRAME = 10_000`) already bound
  this; no new bound needed. The cell's `source.args` captures the source + sql at pin time, so the
  pinned cell is "this query against this source, live" — the right mental model.
- **The "retire the client branch" framing (follow-up #5) is half-right.** RENDERING can be
  descriptor-driven (Slice C closes this half); ROUTING (which payload kind the palette emits)
  carries workflow semantics a descriptor cannot express, so the routing branches stay. This slice
  reframes follow-up #5 explicitly rather than silently leaving a half-truth — see Open questions.
- **No new failure modes.** Slice C adds no new code path — only config consumed by EXISTING
  generic mechanisms. The risk surface is "is the envelope well-formed?" (a unit test per tool) and
  "does the generic pin/render path accept it?" (the headline integration test).

## Open questions

- **The `tools[]` set for a read-only tool.** `reminder.list` declares `tools:[list, update, fire,
  delete]` (the read + its row-control write verbs). `federation.query`/`query.run` have NO
  row-control write verbs — they're pure reads. Decision: `tools[]` is just `[<self>]` (the read
  itself). The bridge leash covers the read; there are no extra write verbs to fold into hidden
  `sources[]`. Verified by the headline test (the minted cell's `sources[]` is empty — the source
  tool is the only one).
- **Reframing rich-responses follow-up #5 (resolved by Slice C).** Follow-up #5 said "Descriptor-
  drive the legacy palette branches" (both `agent.invoke` and `federation.query`). Slice C splits it:
  (a) the RENDERING half — "a tool's answer can mount via the shipped `WidgetView` from a
  descriptor-declared envelope, no hardcoded client render branch" — is **closed** for
  `federation.query` and `query.run` (proven by the UI gateway test); (b) the ROUTING half — "the
  palette emits a specific payload KIND per tool" — is **intentional, not a leak**: the `kind:"query"`
  route carries the async query-worker workflow and the `kind:"agent"` route carries the streaming
  run, neither of which a static descriptor template can replace. The follow-up is reframed as "the
  rendering half is descriptor-driven (done for tabular tools); the routing half is the workflow-
  carrying seam (intentional, documented)." Recorded in `channels-rich-responses-scope.md`'s
  follow-up #5 by this slice.
- **Should `query.run` by-id pin the query id or the resolved text?** A `query.run {id:"daily"}`
  envelope captured at pin time carries `source.args = {id:"daily"}` — the pinned cell re-runs the
  saved query by id, so an edit to the saved query propagates to the dashboard (likely the intent:
  "the daily query, live"). Capturing the resolved text instead would freeze the query at pin time
  (likely NOT the intent). Decision: the envelope carries `{id}` verbatim if the caller pinned by
  id, `{lang,text,target}` verbatim if inline — `query_run` handles both shapes already. Documented.

## Related

- Umbrella: [`widget-platform-scope.md`](widget-platform-scope.md) (the program; Slice C § ~line 116).
- Slice A (shipped): [`../frontend/dashboard/widget-catalog-scope.md`](../frontend/dashboard/widget-catalog-scope.md)
  — `dashboard.catalog` + `check_view_cells` (the validator the pin reuses).
- Slice B (shipped): [`pin-to-dashboard-scope.md`](pin-to-dashboard-scope.md) — `dashboard.pin` +
  `mint_cell_from_envelope` (the generic pin path the headline exercises).
- Precedents reused: [`reminder/descriptor.rs`](../../../rust/crates/host/src/reminder/descriptor.rs)
  `list_render()` (the `result` envelope template), [`tools/descriptor.rs`](../../../rust/crates/host/src/tools/descriptor.rs)
  `host_descriptors()` (the collector), [`viz/frame.rs`](../../../rust/crates/host/src/viz/frame.rs)
  `result_to_rows` (the columnar→rows normalizer that makes `federation.query`/`query.run` source-
  rerun renders work), [`CommandPalette.tsx`](../../../ui/src/features/channel/palette/CommandPalette.tsx)
  the generic `tool.result` branch (the descriptor-driven palette path).
- Closing the rendering half of: [`../channels/channels-rich-responses-scope.md`](../channels/channels-rich-responses-scope.md)
  follow-up #5.
- Core rules: README §3 (rules 5/6/7/10), `docs/scope/extensions/extensions-scope.md` (opaque tool ids).
- Skill (build updates it): [`skills/dashboard-widgets/SKILL.md`](../../skills/dashboard-widgets/SKILL.md).
