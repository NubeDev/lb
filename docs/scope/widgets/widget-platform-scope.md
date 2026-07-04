# Widgets scope — the system-wide widget platform (umbrella program)

Status: **umbrella scope** (the program map — the *why* and the slice sequence, not one buildable ask).
Each slice below is its own build-ready `*-scope.md`. Topic: `widgets`. Promotes shipped truth to
`public/frontend/widget-kit.md` + `public/channels/` + `public/frontend/dashboard.md` (each slice to its
surface's public doc).

**A "widget" is one thing, everywhere.** The platform already has a single render contract — a widget is a
`{ view, source|data, options, action, tools }` envelope, mounted by one renderer
([`WidgetView`](../../../ui/src/features/dashboard/views/WidgetView.tsx)) on every surface: a dashboard grid
cell, a **channel** rich response ([`ResponseView`](../../../ui/src/features/channel/ResponseView.tsx) is a
thin adapter over the same renderer), and — *once its render surface is built* — the **RN app**
([`app/`](../../../app)), which consumes the same backend contracts (`dashboard.catalog` + `dashboard.get`)
but does not render widgets today (Slice A *enables* it; the app page is its own `app/` task). This umbrella
records the whole picture so the individual slices hang off one
frame instead of re-deciding it each time: **widgets are a system-wide capability, not a dashboard feature**,
and the **channel is the integration test-bench** where a user can exercise every tool/API, turn a response
into a widget, preview it, and pin it to a dashboard — all through generic, capability-gated MCP seams.

> **This program does not invent a render system.** It *connects* the ones already shipped
> (channels-rich-responses froze the contract; widget-kit extracted the shared library; genui added the
> AI-authored view). The work is closing the seams between surfaces, and making **every** tool a
> first-class widget — not building a second UI stack.

---

## The model: what a widget is, and where they come from

A widget = the **v2/v3 render envelope** `{ view, source|data, options, action, tools }`, resolved by
**string** and mounted by `WidgetView`. `view` is a built-in kind (`table`/`stat`/`gauge`/…), a scripted
kind, `genui`, or an `ext:<id>/<widget>` federation key. Data reaches it through the host-mediated bridge,
leashed to `cell.tools ∩ the viewer's grant`, re-checked at the host per call (never a token at the widget;
workspace from the token, not the message).

**Four sources of widgets** feed that one renderer:

| # | Source | Declared in | Discovered via | Status |
|---|---|---|---|---|
| 1 | **Built-in view kinds** + per-view config schema | `WidgetView` switch + `widget_catalog.json` | `dashboard.catalog` (Slice A) | building |
| 2 | **Tool result-renders** — a tool declares *how its answer renders* (`ToolDescriptor.result` = the `x-lb-render` envelope). **The reminder widget is this**: `reminder.list` → a `table` view with row-controls. | `<tool>/descriptor.rs` `result` | `tools.catalog` | shipped — `reminder.list` (rich-responses), `federation.query`/`query.run` (Slice C) |
| 3 | **Extension `[[widget]]` tiles** | ext manifest `[[widget]]` | `ext.list.widgets[]` | shipped |
| 4 | **genui** (AI-authored, catalog-constrained IR) | `genui_catalog.json` | `dashboard.catalog` (Slice A) | shipped (dashboard-only) |

The insight the reminder widget makes concrete: **a tool + a `result` envelope *is* a widget** (source #2),
with **no bespoke component** — it renders in a channel today and should be pinnable to a dashboard tomorrow.
"Make widgets reusable across the whole system" = make all four sources discoverable, authorable, and
placeable on **any** surface, through the same generic seams.

---

## What is shipped (the foundation this program builds on)

- **One render contract, two surfaces.** `WidgetView` (dashboard) is reused by `ResponseView` (channel)
  wholesale — same `view` vocabulary, same trust tiers, same bridge (`channels-rich-responses-scope.md`,
  **shipped**). A channel rich response *is* a dashboard cell that isn't on a grid.
- **Descriptors carry BOTH halves of the contract.** `ToolDescriptor { input_schema, result }`
  ([`registry.rs`](../../../rust/crates/mcp/src/registry.rs)): `input_schema` (JSON Schema + `x-lb` hints)
  drives the request FORM; `result` (`x-lb-render`) drives the response WIDGET. `tools.catalog`
  ([`tools/catalog.rs`](../../../rust/crates/host/src/tools/catalog.rs)) exposes both, gated per-tool
  (the tool's own cap decides its visibility — "the menu *is* the permission model").
- **The reminder widget is pure descriptor config.** [`reminder/descriptor.rs`](../../../rust/crates/host/src/reminder/descriptor.rs)
  `list_render()` declares the `table` widget + row-controls (→ `reminder.update`/`fire`/`delete`) + a
  `fieldConfig`. No component. It renders in a channel via the shared renderer.
- **The shared widget library.** `ui/src/lib/widgets/` (widget-kit Phase 1, **shipped**): the registry,
  input widgets, the field-presentation resolver — imported by palette, dashboard, **and** channel.
- **genui.** `@nube/genui` + `view:"genui"` + host-side `genui.rs` validation (**shipped**), wired into the
  dashboard/panel builder.

---

## The gaps this program closes

- **G1 — schema coverage is thin on the OUTPUT half.** `input_schema` is broad, but a `result` render
  envelope was declared by **~one** host tool (`reminder.list`) before Slice C. `federation.query`,
  `agent.invoke`, `query.*` rendered via hardcoded client branches (rich-responses follow-up #5). "Every
  tool/API is a widget with a JSON schema in **and** out" is now true for the **tabular** tools (Slice C
  gave `federation.query` and `query.run` a `result = table` envelope); `agent.invoke` is deferred to
  Slice D (streaming/nondeterministic — the snapshot model fits, not source-rerun), and `query.save`/
  `query.compile` are named follow-ups. Follow-up #5's RENDERING half is closed for the tabular tools;
  its ROUTING half is intentional (the palette's `kind:"query"`/`kind:"agent"` branches carry async/
  streaming workflow semantics a static descriptor cannot express).
- **G2 — a tool/channel widget cannot be pinned to a dashboard.** A `result` render is **ephemeral in a
  channel** (lives in the channel `Item` body, not a `dashboard:{id}` cell). Nothing mints a persisted cell
  from a tool's render — so the **reminder widget cannot be added to a dashboard** (rich-responses
  follow-up #2, unbuilt). *This is the direct payoff for the headline example.*
- **G3 — no channel-origin authoring.** "Query the system → ask the AI to turn the last response into a
  widget → preview it → add it to a dashboard" does not exist. genui authoring is dashboard-builder-only
  (genui Decision 3 deferred the channel tenant).
- **G4 — no discovery of the built-in view palette + config, and no gate on bad views.** The AI invents
  `view` kinds / config that don't exist and `dashboard.save` accepts them (except `genui`). *(This is the
  original bug that kicked off the program.)*
- **G5 — extension capability introspection is partial.** `ext.list` carries a tile's `scope`, but there
  is no focused "what can this extension do, and what has it been granted" read for a user/admin/AI to
  reason about before wiring an extension's tool/widget in.

---

## The slices (the program sequence)

Each is (or becomes) its own build-ready scope. Ordered so each unblocks the next; **Slice A is in flight**.

### Slice A — Widget catalog + save-validation (closes G4) — *in flight*
`dashboard.catalog` MCP verb: the built-in view palette (each view with a **per-widget version**, `kind`,
`data`/`action`, and its config-field schema) + the ext `[[widget]]` tiles + genui components, from a
**host-owned `widget_catalog.json`** (backend-driven, client-agnostic — web, AI, app read one authority).
Plus host-side `dashboard.save` validation rejecting a cell with an **unknown `view`**. Fixes the
hallucinated-widget bug and gives the AI/app the built-in-view menu. Full scope:
[`../frontend/dashboard/widget-catalog-scope.md`](../frontend/dashboard/widget-catalog-scope.md).

### Slice B — Pin-to-dashboard: mint a cell from a tool result-render (closes G2) — *shipped 2026-07-04*
The keystone for "widgets are system-wide." A generic path that takes any `x-lb-render` envelope (a tool's
`descriptor.result`, or a live channel `rich_result` body) and mints a persisted `dashboard:{id}` cell via
the new `dashboard.pin` verb (a server-side mint — the umbrella's open question, resolved; see below).
**The reminder widget becomes dashboard-addable** with zero reminder-specific code — the envelope is
already a valid `{view:"table", source:{tool:"reminder.list"}, options, tools}` cell-shape, and Slice A's
`check_view_cells` validator already accepts it. No branch on a tool id (rule 10) — the envelope is opaque
data. Named in rich-responses as follow-up #2. Full scope:
[`pin-to-dashboard-scope.md`](pin-to-dashboard-scope.md); session
[`../../sessions/widgets/pin-to-dashboard-session.md`](../../sessions/widgets/pin-to-dashboard-session.md).

### Slice C — Result-render coverage: every tool declares its output widget (closes G1) — *shipped 2026-07-04*
Give the remaining host tools a `descriptor.result` envelope (start with `federation.query` → `table`,
`agent.invoke` → its render, `query.run` → `table`) so the channel renders them descriptor-driven, not via
hardcoded client branches — retiring rich-responses follow-up #5. Each new envelope is backend config; the
generic palette + `WidgetView` render it with no UI change. This is also what makes **every** tool
pin-able (Slice B) and app-renderable. **SHIPPED for the tabular tools (`federation.query`, `query.run`):
each carries a `result = table` envelope; `agent.invoke` is DEFERRED to Slice D** (its streaming +
nondeterministic render belongs to Slice D's snapshot model, not the source-rerun model — see the slice
scope's "Why agent.invoke is deferred"). Follow-up #5 is **reframed by Slice C**: the RENDERING half is
descriptor-driven for the tabular tools (closed); the ROUTING half (which payload KIND the palette emits)
is intentional workflow-carrying seam, not a leak. Full scope:
[`result-render-coverage-scope.md`](result-render-coverage-scope.md); session
[`../../sessions/widgets/result-render-coverage-session.md`](../../sessions/widgets/result-render-coverage-session.md).

### Slice D — Channel-origin authoring: response → widget → preview → dashboard (closes G3)
The user-facing through-line. From a channel: (1) take the last `rich_result` (or a query result), (2) ask
the AI to author a widget over it — including a `view:"genui"` widget (wire genui as the channel tenant
genui Decision 3 reserved), (3) preview it live through the same `WidgetView`, (4) pin it to a dashboard
(Slice B). Depends on B + genui-in-channel.

### Slice E — Extension capability introspection (closes G5)
A focused read (`ext.caps`/extend `ext.list`) surfacing, per installed extension, its declared vs
admin-approved vs granted capabilities and the tools/widgets it contributes — so a user/admin/AI can see
"what can this extension do" before wiring it. Generic over the id (rule 10); reads the existing install
grant, invents no new capability.

---

## The through-line (the user's scenario, end to end)

The scenario this program enables, and why the **channel is the test-bench for the whole system**:

1. **Query.** In a channel, the user runs a SQL query (`federation.query`) or calls any tool. It answers
   with a typed widget (Slice C gives it a `result` render).
2. **Make a widget from the last response.** The user asks the AI: "turn that into a stat widget." The AI
   reads `dashboard.catalog` (Slice A — which views exist + config) and `tools.catalog` (which tool renders
   exist), authors an envelope (a fixed view, or a `genui` widget for a novel layout), and the channel
   **previews** it live via `WidgetView` (Slice D).
3. **Add it to a dashboard.** The user says "add this to the Ops dashboard." The AI (or a UI affordance)
   pins the previewed envelope as a persisted cell via `dashboard.save` (Slice B). Slice A's validator
   confirms the `view` is real; the cell renders on the grid exactly as it did in the channel.
4. **The reminder widget, same path.** `reminder.list` already declares its widget (source #2); Slice B
   makes it pinnable — "add the reminder widget to my dashboard" works with no reminder-specific code.
5. **Understand an extension first.** Before wiring an extension's tool, the user asks "what can this
   extension do?" — Slice E answers from the install grant.

Every step is a **generic MCP seam** (`tools.catalog`, `dashboard.catalog`, `dashboard.save`, `ext.list`) —
the client holds no tool- or extension-specific knowledge (rule 7 + rule 10).

---

## How it fits the core

- **MCP is the universal contract (rule 7).** Every seam is an MCP verb read/consumed identically by the
  web UI, the channel, the RN app, and AI agents. No surface holds tool-specific rendering knowledge; new
  widgets ship as backend/extension config.
- **Backend-driven rendering = two documents, composed (the app-render contract).** "A surface knows what
  to render" is *two* backend answers, never one: the **catalog** (`dashboard.catalog`, Slice A) supplies
  the *vocabulary* — what `view` kinds exist and how each is configured — and the **page/response document**
  (`dashboard.get` for a grid; a channel `rich_result` for a response) supplies the *content* — which cells,
  which `view` each, its `options`, its data `source`. A client renders by walking the document, resolving
  each `view` against the catalog, and binding data through the one gated bridge. This is what lets the
  **RN app** render a page with **no app-side palette and no code release** when a view or ext tile is added
  — the app holds the same zero tool/ext knowledge the web shell does. Slice A ships the catalog half;
  `dashboard.get` is shipped; the app's *renderer* (the `view`→native-component map) is the app task's own
  work. The full contract + its honest gaps (ext-tile config, unknown-option tolerance) live in the Slice A
  scope's "How the app renders a page".
- **Core knows no extension (rule 10).** Sources #2/#3 and Slices B/E treat tool ids and extension ids as
  **opaque data** — a render envelope, an `ext.list` row, a capability string — never a branch on a named
  id. A GitLab-for-GitHub swap forces no core change.
- **Capability-first (rule 5) + workspace wall (rule 6).** Every widget's data call is the viewer's grant
  ∩ the cell's declared tools, re-checked at the host; workspace from the token. Discovery verbs
  (`*.catalog`) leak only what the caller is granted. Pinning invents no capability — it *calls*
  already-gated ones.
- **State vs motion (rule 3).** A pinned widget is state (`dashboard:{id}` cell); a channel response is
  motion+history (the channel `Item`). Slice B is the adapter between them, not a new persistence layer.

## Testing plan

The **channel is the integration bench** — the mandatory categories play out there against a real gateway
(rule 9): capability-deny (a viewer without a tool's cap can't render/pin its widget), workspace-isolation
(a ws-B widget reaches only ws-B), and the end-to-end through-line (query → author → preview → pin →
render-on-grid) as a real-gateway E2E once Slices A–D land. Each slice carries its own scoped tests; this
umbrella owns the **cross-surface** test: the *same* envelope renders identically in a channel and a pinned
dashboard cell.

## Risks & hard problems

- **Keeping the client generic.** The temptation at each seam is a tool-name/ext-id branch in the UI to
  "just make it work." That is the exact leak channels-rich-responses already fought (its "Resolved design
  correction"). Every slice must route through the descriptor/catalog/grant seams.
- **Envelope ↔ cell fidelity (Slice B).** A `result` envelope and a persisted `Cell` are close but not
  identical (a cell has layout `i,x,y,w,h`, a `panel_ref`, `schema_version`). The mint must produce a valid
  v3 cell that Slice A's validator accepts and `WidgetView` renders unchanged.
- **genui in a channel (Slice D)** inherits genui's iframe/trust story; do not weaken it to fit the channel.

## Open questions

- **One catalog or two?** `dashboard.catalog` (view kinds + ext tiles + genui components) vs
  `tools.catalog` (callable tools + their result-renders) are distinct concerns the AI reads together.
  Keep separate (recommended — a render kind is not a tool, and `dashboard.catalog` self-describes via a
  `ToolDescriptor` so it's discoverable *from* `tools.catalog`) or a future `widget.catalog` façade that
  composes both? Decide when the AI-authoring prompt (Slice D) is built — and that Slice D scope must open
  with the "read both catalogs; this one answers *what can render*, that one answers *what can be called
  and how its answer renders*" table, so the split is taught, not rediscovered.
- **Where does "pin" live?** A `dashboard.pin(render)` verb, or the client builds the cell and calls
  `dashboard.save`? **RESOLVED in Slice B (`pin-to-dashboard-scope.md`): a server-side `dashboard.pin`
  mint verb.** The proof of necessity is the same argument Slice A used to put save-validation server-side:
  a pin produces persisted state, and a headless `POST /mcp/call` agent (no shell, no
  `ResponseView.buildCell`) must be able to pin a tool's `result` envelope — with client-compose every
  client re-implements the envelope→cell mapping and the host can't enforce fidelity. The verb mints the
  cell host-side (generic over the tool id, rule 10) and reuses the Slice A validation chain; the channel
  render path (`ResponseView.buildCell`) is untouched (it keeps doing ephemeral envelope→cell for render;
  `dashboard.pin` is the persist-time twin). See
  [`pin-to-dashboard-scope.md`](pin-to-dashboard-scope.md) for the full reasoning.
- **Per-widget version consumption.** Declared in Slice A; stamping + migration deferred until a widget
  gets a breaking v2 (dev mode).

## Related

- Shipped foundation: `channels/channels-rich-responses-scope.md` (the contract + follow-ups #2/#5),
  `frontend/widget-kit-scope.md` (the shared library), `genui/genui-scope.md` (the AI view + Decision 3),
  `frontend/dashboard/widget-builder-scope.md` (the v2 cell contract).
- Slices: [A](../frontend/dashboard/widget-catalog-scope.md) (in flight); B–E named above (scopes written
  as each is picked up).
- Renderer: [`WidgetView.tsx`](../../../ui/src/features/dashboard/views/WidgetView.tsx),
  [`ResponseView.tsx`](../../../ui/src/features/channel/ResponseView.tsx). Contract:
  [`registry.rs`](../../../rust/crates/mcp/src/registry.rs) (`ToolDescriptor`).
- Core rules: README §3 (rules 3/5/6/7/10).
