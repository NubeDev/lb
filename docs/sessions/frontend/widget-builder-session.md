# Session — the tool-driven widget builder (dashboard v2: any view → any MCP tool)

Topic: `frontend` · Scope: [widget-builder-scope.md](../../scope/frontend/dashboard/widget-builder-scope.md) ·
Date: 2026-06-27 · State: **shipped**

## The ask

Build the **tool-driven widget builder** from `widget-builder-scope.md`: generalize a dashboard cell
from "a read-only renderer bound to one series" (the frozen v1 contract) to **"any *view* bound to any
*MCP tool call* the install grant allows — read or write."** A user picks a data source that is just an
MCP tool call (hidden behind a friendly **source picker**), maps it into a view (chart/stat/gauge/table,
Observable Plot, D3, a JSX template), or drops a control (switch/slider/button) that *calls* a write
tool, and saves it as a dashboard cell. Authored three ways — configured in-app, scripted in-app (an
inline Plot/D3/JSX template), or shipped by an extension developer (`[[widget]]`). Every path rides the
**one bridge** (`bridge.call(tool, args)` + `bridge.watch(...)`), leashed by the install grant and
re-checked at the host per call. This makes the dashboard a generic front-end for the MCP tool surface
(rule 7).

The scope was **fully decided** — built as written; the "lean: X" follow-ups were resolved as the scope
suggested (recorded below).

## What shipped (vertical slices, backend → frontend)

### Slice 1 — the cell v2 contract + render_templates CRUD (backend)

- **Cell record → v2** (`crates/host/src/dashboard/model.rs`): added serde-defaulted `v`, `view`,
  `source { tool, args }`, and `action { tool, args_template }` fields. A v1 series cell deserializes
  unchanged (a v1 cell is a v2 cell whose tool set is the four series read verbs). `Source`/`Action`
  exported (`CellSource` at the crate boundary to avoid the registry `Source` clash).
- **`render_templates` table + CRUD** (`crates/host/src/render_templates/`, one verb per file): a
  workspace-scoped, **author-owned** `render_template:{id}` record holding a durable scripted-view
  snippet (Plot/D3/JSX) larger than the inline `cell.options` cap. Verbs `template.save` (idempotent
  UPSERT, author-only update, size-capped), `template.get` (workspace-shared read), `template.list`
  (roster summaries, no code bodies), `template.delete` (idempotent tombstone, author-only). Gated
  `mcp:template.<verb>:call` (workspace-first, then capability); denials opaque. Wired into the
  `call_tool` bridge dispatch (`is_host_native` now also matches `dashboard.`/`template.`) and the dev
  claim set.
- **Tests** (`crates/host/tests/render_templates_test.rs`, 6): CRUD round-trip, deny-per-verb,
  ws-isolation, author-ownership (a non-author cannot overwrite/delete), the size cap, upsert
  idempotency (offline/sync). All green.

### Slice 2 — WidgetBridge v2 + the renderers' trust tiers (frontend)

- **WidgetBridge v2** (`ui/src/features/dashboard/builder/widgetBridge.ts`): `call(tool, args)` forwards
  ANY tool in `cell.tools ∩ grant` (read OR write; the local scope filter is defense-in-depth, the host
  re-checks), plus `watch(tool, args, onEvent) => unsubscribe` mapping `series.watch`/`bus.watch` onto
  the **shipped series SSE** (no new transport, no polling). **The token never crosses the bridge** — it
  rides server-side in `invoke`/EventSource only.
- **Sandboxed-iframe runtime** (`builder/iframeRuntime.ts` + `builder/WidgetIframe.tsx`): scripted views
  (Plot/D3/JSX `template`) and untrusted extension widgets render in an **opaque-origin iframe**
  (`sandbox="allow-scripts"`, NO `allow-same-origin`) with a CSP. The frame reaches data only by
  `postMessage`ing `{tool,args}` to the parent, which re-checks the tool set and forwards through the
  bridge; the token never enters the frame (srcdoc, reply, or watch event). A scripted view MAY write a
  granted tool — the sandbox + the grant + the host re-check are the three guards.
- **Trust-tier routing** (`builder/trust.ts`): an allow-listed publisher key → in-process module
  federation; everything else + all scripted views → iframe. Allow-list is shell config
  (`VITE_TRUSTED_WIDGET_KEYS`), **empty by default** (safe — in-process is opt-in).
- **`ext:<id>/<widget>` renderer** (`builder/ExtWidget.tsx` + `builder/federationWidget.ts`): mounts an
  extension tile, modelled on `proof-panel`; trusted key → in-process `mountWidget` (a named export on
  the same remote, open Q2); else loads the remote inside the iframe sandbox. Uninstall → "extension not
  installed", streams torn down (stateless eviction).

### Slice 3 — the view renderers + the builder UI

- **View renderers** (`ui/src/features/dashboard/views/`): `ChartView`/`StatView`/`GaugeView`/`TableView`
  (read, over the new generic `useSource` hook that runs the source through the bridge and introspects
  the result shape into rows/latest — the rubix-cube `transformDataToColumns` analog with the data layer
  swapped to the bridge); `ScriptedView` (plot/d3/template via the iframe, inline `options.code` or a
  durable `template.get` by `options.templateId`); `SwitchControl`/`SliderControl`/`ButtonControl` (call
  a write tool via the bridge, filling a typed `argsTemplate` `{{value}}` slot — open Q4); `WidgetView`
  dispatches the whole vocabulary. `WidgetHost` routes v1 cells to the legacy widgets and v2 cells
  (`v:2`/`view`/`source`) to `WidgetView`.
- **The builder** (`builder/WidgetBuilder.tsx` + `sourcePicker.ts` + `useSourcePicker.ts`): the
  **source picker** assembles friendly entries from the shipped `series.list` + `ext.list` (Series /
  Live-Zenoh / installed-extension / Action), each resolving a label to `{tool,args}` — **the author
  never sees a tool name**. Pick a source → choose a view (only those valid for the source shape) →
  inline code for scripted views → live preview through the real bridge → **Add** appends a v2 cell
  persisted via the unchanged `dashboard.save`. Replaced the v1 `AddWidget` (deleted).
- **`template.api.ts`**: the `template.*` client over the `mcp_call` bridge.

### Reference extension — `proof-panel` ships a `[[widget]]`

Resolved the proof-panel scope's deferred `[[widget]]` open question: a `[[widget]]` tile in
`extension.toml` + a SECOND named `mountWidget` export on the same `remoteEntry.js` (open Q2: one
build) rendering a compact `WidgetTile` that reads `proof.demo`'s latest through the v2 bridge. This is
the working model for an extension-shipped widget.

## Open-question follow-ups — resolved (per the scope's "lean")

1. **ext cell key** = `ext:<id>/<widget-id>` (`ExtWidget.parseExtKey`).
2. **widget expose** = a named `mountWidget` export on the same remote entry (one build).
3. **inline-vs-row template threshold** = a small inline cap (`INLINE_MAX_BYTES = 4 KB`); larger ⇒ a
   `render_template:{id}` row (`TEMPLATE_MAX_BYTES = 64 KB` hard cap).
4. **control args** = a typed `args_template`/`argsTemplate` with one `{{value}}` slot
   (`views/argsTemplate.ts`, type-preserving substitution).
5. **control reads its own state** = yes, optional (`SwitchControl` reflects an optional `source`).

## Tests — all green (real gateway, real store, real caps; no fakes)

**Backend (Rust):** 6 `render_templates_test` + 23 `proof_panel_test` (manifest with `[[widget]]` still
installs/loads) + 5 `dashboard_test` (v1 cell literal updated for v2 fields). Workspace:
**404 passed** (`cargo test --workspace`; the 1 `offline_sync` Zenoh-timing flake passes in isolation,
pre-existing/untouched). `cargo fmt` + `cargo build --workspace` clean.

**Frontend:** **9** pure-logic unit tests (`builder/widgetBuilder.test.ts`: source-picker mapping,
typed argsTemplate fill, trust-tier default) + **11** real-gateway tests
(`builder/widgetBuilder.gateway.test.tsx`):

- render_templates CRUD round-trip + deny-per-verb + ws-isolation;
- **capability deny incl. WRITES** — an ungranted `ingest.write` denied **server-side even with the
  bridge filter bypassed** (the grant is the real leash); a tool outside the cell set denied at the
  bridge;
- **ws-isolation across a write** — a ws-B write widget cannot write into ws-A;
- **token never crosses the boundary** — no session token in any forwarded bridge argument;
- **write-control e2e** — a control's write produces a real, readable side effect (the written sample
  reads back);
- **scripted-template write deny** when the tool is ungranted;
- **trust-tier routing** — a non-allow-listed ext widget renders sandboxed (iframe), never in-process;
- **extension-widget e2e** — install with a `[[widget]]` → palette tile (read/write split by label) →
  uninstall evicts.

Plus the updated `DashboardView.gateway.test.tsx` (3) drives the new builder (source-pick a seeded
series → chart/stat over the bridge → persist). UI totals: **36 unit + 96 gateway green**. `tsc` clean,
0 eslint errors. `proof-panel` `build.sh` green (remoteEntry 101 kB); 12 proof-panel UI tests green.

## Decisions & rejected alternatives

- **Template CRUD rides the `mcp_call` bridge, not bespoke REST routes.** The builder *consumes* tools
  (the scope's framing); a dedicated `/templates` REST surface would duplicate the one MCP contract. The
  host dispatches `template.*` in `call_tool`, reachable over `POST /mcp/call` like any tool.
- **Trust allow-list defaults EMPTY.** Safe by construction: every widget iframes unless the shell
  explicitly trusts a publisher key. In-process federation is the opt-in, never the default — the one
  thing the scope says we do not bend (arbitrary author code in the shell process is RCE).
- **`useSource` introspects the result shape** (`{samples}`/array/`{value}`/scalar) rather than assuming
  a series shape — so a chart/table works over *any* read tool, not just `series.read`.
- **Did not generate shadcn Select/Textarea primitives** (out of this slice's scope; only
  Button/Input/Card/… exist). The picker/code-editor use native elements with the Input-matching token
  class + a per-element justified `eslint-disable`; the Button primitive is used where it exists.

## Debugging

No non-trivial bug needed a `debugging/` entry. Two test-author fixes (caught by the real gateway, not
mocks): `Sample` requires a `producer` field (the host overwrites it but serde needs it present); the
source-picker options load async via `series.list`, so the gateway test waits for the option before
selecting. Both are test correctness, not product bugs.

## Cross-links

- Scope: [widget-builder-scope.md](../../scope/frontend/dashboard/widget-builder-scope.md) (open
  questions resolved there)
- Public: [public/frontend/dashboard.md](../../public/frontend/dashboard.md) (v2 promoted)
- Supersedes the v1 contract in
  [dashboard-widgets-scope.md](../../scope/frontend/dashboard-widgets-scope.md) /
  [dashboard/widgets-scope.md](../../scope/frontend/dashboard/widgets-scope.md) (the `ext:<id>` follow-up
  built here).
