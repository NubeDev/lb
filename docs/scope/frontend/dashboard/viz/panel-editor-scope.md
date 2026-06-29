# Viz scope — the panel editor (one Grafana-style editor, add == edit)

Status: **Phase 1 shipped (2026-06-29)** — the ONE `PanelEditor` (add ≡ edit, the pinned pure
`cell ↔ editorState` round-trip) with the full Query/Transform/Panel options/Field/Overrides tab
structure landed; `WidgetBuilder`+`CellSettings` retired from the dashboard path. The headline "edit
loses my SQL options / add ≠ edit" bug is fixed (the Transform tab is the Phase-1 config shell — no
client transforms, per invariant B). Part of the [`viz/`](README.md) slice — the **UX surface** that
authors the [panel model](panel-model-scope.md). Shipped truth in
[`public/frontend/dashboard.md`](../../../../public/frontend/dashboard.md); session:
[`dashboard-viz-phase1`](../../../../sessions/frontend/dashboard-viz-phase1-session.md).

One paragraph: replace the cramped, inconsistent widget builder with **one** Grafana-style panel editor —
a full-surface editor (live preview + visualization picker + a right-hand options rail of tabs: **Query**,
**Transform**, **Panel options**, **Field**, **Thresholds**, **Value mappings**, **Overrides**) that renders
the **complete** [panel-model](panel-model-scope.md) option set **from the cell**, identically whether the
cell is new (an empty/default cell) or existing (the saved cell). This kills the user's three complaints in
one move: the options are *bad* → we surface the full `fieldConfig`/per-view/`transformations` taxonomy; the
options *differ between add and edit and editing loses my SQL builder state* → there is **one** field-code
path and **one** `cell ↔ editor-state` (de)serializer, round-trip tested, so nothing can drift or be dropped;
the UX is *ugly* → it is rebuilt on shadcn primitives in the canonical [`ui-standards`](../../ui-standards-scope.md)
look. This is a **frontend-only** slice: no host verb, no capability, no datastore change — it rides
`dashboard.save`/`get`.

## Goals

- **One editor for add and edit.** A single `PanelEditor` mounted on a cell. *Add* passes an empty/default
  cell; *edit* passes the existing cell. **Same component, same code path** — so the option surface and the
  round-trip are provably identical. (This is the defect the user hit; it is the headline of this scope.)
- **Surface the COMPLETE option model.** Every field defined by [`panel-model`](panel-model-scope.md) is
  editable: `view` (the viz picker), `sources[]` (targets + datasource + SQL Builder⇄Code), `transformations[]`,
  per-view `options`, `fieldConfig.defaults` (unit/decimals/min/max/color/noValue/thresholds/mappings), and
  `fieldConfig.overrides[]`. No option group is add-only or edit-only.
- **Reconstruct ALL of it on edit.** A `cellEditorState` (de)serializer rebuilds *every* group from the saved
  cell — notably the SQL Builder state in `cell.options.sql` (raw + builder query), `fieldConfig`,
  `transformations`, and per-view `options` — so reopening the editor shows exactly what was saved.
- **Adopt Grafana's proven editor layout**, not a new one: preview pane, viz picker, tabbed options rail with
  collapsible groups and an options **search** filter.
- **Live preview that is the real thing.** The preview resolves the draft panel through the backend
  **`viz.query`** verb (real rows + the transformation pipeline applied server-side by `lb-viz`), then
  applies `fieldConfig` formatting and renders the chosen `view` — exactly what `save` will persist. Preview
  is a **debounced `viz.query`** on each edit, so the editor uses the *same* resolver as render (no
  client-side transform copy). (In Phase 1, before `viz.query` ships, a no-transform preview runs the source
  over the shipped v2 bridge behind the one data hook — see the umbrella phasing.)
- **shadcn-first, canonical look.** Rebuilt on shadcn primitives per [`ui-standards`](../../ui-standards-scope.md);
  responsive; the Members/NavRail aesthetic. No native `<select>`, no squished inline bar.

## Non-goals

- **No new option *semantics*.** What each field means/renders is owned by [`field-config`](field-config-scope.md),
  [`chart-types`](chart-types-scope.md), [`transformations`](transformations-scope.md). This doc only *exposes*
  them and guarantees the round-trip.
- **No new datasource plane / no raw handles.** The datasource dropdown and target resolution are
  [`datasource-binding`](datasource-binding-scope.md); a target is always a host-gated MCP tool call.
- **No host change.** No new verb, no new capability, no datastore migration. (Stated below under *How it fits*.)
- **No Grafana plugin runtime.** We adopt the editor's *layout and option taxonomy*, not its React/Angular SDK.
- **Not the import/export UI.** Paste/upload/download a Grafana dashboard is [`import-export`](import-export-scope.md).

## Intent / approach

**Diagnosis (the root cause).** The shipped editor — `ui/src/features/dashboard/builder/WidgetBuilder.tsx`
(~329 lines) plus the `CellSettings.tsx` (~108 line) ⚙ Sheet — has **two** defects that combine into the
user's report:

1. **The option surface is incomplete.** It exposes only the minimal legacy options (chart=unit, stat=unit,
   gauge=min/max/unit). The new panel-model groups (`fieldConfig`, per-view `options`, `transformations`,
   overrides) have nowhere to live — so the options are "really bad."
2. **The seed/round-trip is partial.** Add mounts `WidgetBuilder` with no seed; edit mounts it with `seed: Cell`.
   The two paths reconstruct *different* subsets of state — and some groups (notably the SQL Builder state in
   `cell.options.sql`, but also any new field-config/per-view option) are **not** fully rebuilt on edit. So a
   user who built a chart with the SQL Builder, saved it, and reopened it "doesn't get those options back."

The public `dashboard.md` *claims* "add and edit share one set of field code and cannot drift." The code does
not honor that claim today. **This scope makes the claim true.**

**The fix.** One `PanelEditor` component renders the complete option model **from the cell**, via a single
pure `cellEditorState` (de)serializer:

- `cellToEditorState(cell) → EditorState` — rebuilds *every* group (viz, targets incl. SQL Builder raw+builder
  query, transformations, per-view options, fieldConfig defaults + overrides).
- `editorStateToCell(state, base) → Cell` — serializes back, preserving the cell **key + geometry** (the edit
  invariant) and emitting the additive v3 fields.
- **Add** = `cellToEditorState(defaultCell(view))`; **edit** = `cellToEditorState(savedCell)`. Identical path.
  Because both ends are the same pure function pair, `editorStateToCell(cellToEditorState(c)) ≡ c` is a unit
  test — drift becomes impossible, not merely discouraged.

**Rejected alternative — patch the existing add-form + edit-drawer.** We considered keeping the two surfaces
and "just adding the missing fields to both." Rejected: it perpetuates the very thing that caused the bug —
*two* places that must be kept in lockstep by hand. The next added option group would drift again. The only
durable fix is **one** surface and **one** (de)serializer, with the round-trip pinned by a test. We also
rejected building a brand-new option vocabulary: we adopt Grafana's editor layout and the panel-model
taxonomy so the map to import/export stays 1:1 and the user's Grafana muscle-memory transfers.

## How it fits the core

This is a **frontend-only editor slice.** Most core concerns are unchanged; stated so explicitly.

- **Tenancy / isolation (rule 6):** unchanged. The editor reads/writes the workspace-scoped `dashboard:{id}`
  cell; the datasource dropdown and source lists are **ws-scoped** (resolved from the token, per
  [`datasource-binding`](datasource-binding-scope.md)) — the editor can never name a foreign-workspace source.
- **Capabilities (rule 5/7):** **none added.** The editor is gated by the existing edit cap
  `mcp:dashboard.save:call`; a viewer without it sees **no** editor entry point (the gate is the backstop, not
  the only check). The host **re-checks `dashboard.save` on save** — the client gate is convenience, the host
  is authority. Per-target reads in the preview reuse the target tool's cap ∩ grant (unchanged leash).
- **Placement (rule 1):** one editor, **two transports** — Tauri `invoke` on desktop / gateway SSE+HTTP in the
  browser, behind the shipped v2 bridge. No `if cloud`.
- **MCP surface (§6.1):** none added. A save is the existing **one synchronous, bounded** `dashboard.save`
  UPSERT of the **whole cell** (including the new v3 fields). Multiple targets in the preview are normal
  per-target `bridge.call`s, leashed as today — not a batch.
- **Data (SurrealDB):** unchanged — one datastore. The cell (with `fieldConfig`/`transformations`/`sources[]`)
  is one record; bounds (`overrides[]`/`transformations[]` caps, inline-code size) are the panel-model rule, not
  re-litigated here.
- **Bus / state-vs-motion:** unchanged. Live preview samples ride the shipped series/bus SSE; the cell is state.
- **Sync / authority:** unchanged — save is the shipped `(table,id)` UPSERT; additive fields replay idempotently.
- **Secrets:** none reach the editor — a federation target's DSN stays server-side (datasource doctrine).
- **SDK/WIT impact:** none. This consumes the v3 cell contract from [`panel-model`](panel-model-scope.md); it
  defines no new contract of its own.

## Example flow

1. A user clicks **Edit** on a saved `timeseries` panel (or **Add panel** → an empty cell). The edit cap
   `mcp:dashboard.save:call` is present, so the editor opens; the full-surface `PanelEditor` mounts on the cell.
2. `cellToEditorState(cell)` reconstructs **everything**: the viz = `timeseries`, the targets (with the SQL
   Builder showing the *builder* query, not just raw SQL, from `cell.options.sql`), the `reduce` transformation,
   the per-view legend/tooltip options, and `fieldConfig.defaults` (unit `celsius`, decimals `1`, a 5 °C
   threshold). **Nothing is missing** — the bug the user hit is gone.
3. The **left preview** runs the real source over the bridge (real rows), applies the transformation +
   fieldConfig, and renders the chart — the same render dispatch (`WidgetView`/`WidgetHost`) that `save` will use.
4. The user opens the **Field** tab, changes decimals to `2`; the preview updates live. They type "threshold"
   into the **options search** — the rail filters to the threshold group.
5. They switch the **viz picker** to `barchart`; the picker offers only views valid for the current data shape;
   per-view options re-render; the targets/fieldConfig are preserved across the switch.
6. **Save** → `editorStateToCell(state, base)` emits the whole v3 cell, **preserving the key + geometry**, and
   `dashboard.save` UPSERTs it. The host re-checks the cap. A reopen re-reads it — identical, by the round-trip test.

## Testing plan

Per [`../../../testing/testing-scope.md`](../../../testing/testing-scope.md) — real gateway, real store, seeded
real rows, **no `*.fake.ts`**. Frontend tests are `*.gateway.test.tsx` against the real spawned gateway.

- **ADD vs EDIT parity (the headline, the user's bug).** In one real-gateway test: build a panel with a **SQL
  Builder** query *and* a `fieldConfig` (unit + decimals + a threshold) *and* a `transformation` *and* per-view
  options; `dashboard.save`; **reopen the editor on the saved cell**; assert **every** option round-trips
  identically — viz, the SQL **Builder** state (not just raw SQL), fieldConfig, transformation, per-view options.
  This is the regression test for "editing loses my SQL options."
- **`cell ↔ editorState` round-trip (unit).** `editorStateToCell(cellToEditorState(c)) ≡ c` for: a v1 cell, a v2
  `chart`+`store.query` cell, and a full v3 cell (sources[]/fieldConfig/transformations/overrides). Pure, no gateway.
- **Live preview is real.** The preview renders **real seeded rows** through the chosen view; and **degrades
  honestly** — a denied target shows a denied/empty/error state, never a fabricated value (rule 9).
- **Viz picker validity.** For a given data shape the picker offers **only** valid views (e.g. a single-row
  reduce result suggests `stat`/`gauge`, not `table`-only) — driven by the [`chart-types`](chart-types-scope.md)
  result-shape↔type validation.
- **Edit-cap gate + host backstop.** A viewer **without** `mcp:dashboard.save:call` sees **no** editor entry
  point; and a forced `dashboard.save` for that identity is **denied by the host** (opaque) — the backstop holds.
- **Workspace isolation.** The editor's datasource dropdown and source lists in ws-B contain **only** ws-B
  sources; a ws-B editor can never resolve a ws-A datasource ref (two-session test).

## Risks & hard problems

- **The (de)serializer is the whole ballgame.** If `cellToEditorState`/`editorStateToCell` miss a field, the
  user's exact bug returns silently. Mitigation: the round-trip identity test is **mandatory** and runs over
  v1/v2/v3 fixtures; new option groups must extend it in the same PR.
- **SQL Builder state reconstruction.** `cell.options.sql` stores **both** raw SQL and the structured builder
  query; edit must rehydrate the *builder*, not collapse to Code-only (the precise thing that's broken now).
  One adapter, tested both directions.
- **Viz switch must preserve compatible state.** Changing `view` should keep targets/fieldConfig and only
  reset genuinely view-specific `options` — losing the query on a viz switch would feel as broken as the
  original bug. Define the carry-over set per [`chart-types`](chart-types-scope.md).
- **Preview cost.** Live preview re-queries on edit; debounce + reuse the shipped preview/RefreshControl
  mechanism, and never busy-loop a target (the transformations bound applies).
- **File-size discipline.** A Grafana-grade editor is large; it must stay one-responsibility-per-file (below) —
  the temptation to grow a 1000-line `PanelEditor` is the real risk to the rule-8 line.

## Resolved decisions

No blocking open questions — these are the long-term answers the build follows.

- **A full-surface Sheet/Dialog in Phase 1** (fast, no route plumbing), with a `?edit=<cellId>` deep-link
  via [`routing`](../../routing-scope.md) as a fast-follow so an open editor is shareable/refresh-safe.
- **Options search filters option labels across all tabs** (Grafana's behavior), surfacing matches inline;
  fuzzy/synonym search is deferred.
- **Ship the full tab structure from day one** — Query / Transform / Panel options / Field / Overrides
  (empty groups collapse) — so adding a Phase-2 option never reintroduces an add/edit fork.
- **Retire `CellSettings` (⚙).** A single editor is the whole point; a separate quick-settings drawer is
  exactly the second surface that drifts. Quick edits live in the one editor.

## Implementation note — file layout (one responsibility per file)

New: `ui/src/features/dashboard/editor/` — `PanelEditor.tsx` (the shell), `VizPicker.tsx`,
`tabs/QueryTab.tsx`, `tabs/TransformTab.tsx`, `tabs/PanelOptionsTab.tsx`, `tabs/FieldTab.tsx`,
`tabs/OverridesTab.tsx`, `OptionsSearch.tsx`, `PreviewPane.tsx`, and `cellEditorState.ts` (the pure
`cell ↔ state` (de)serializer). Each ≤400 lines (rule 8). The new editor **supersedes** `builder/WidgetBuilder.tsx`
+ `CellSettings.tsx`; it **reuses, not rebuilds**: the source picker (friendly labels → `{tool,args}`), the SQL
Builder⇄Code editor (`builder/sql/`, `SqlBuilderQuery` + `toSurrealQL`), the CodeMirror editors
(`CodeEditor`/`PlotCodeField`/`TemplateSourceField`/`SqlEditor`), the JSON payload builder, the vars editor/bar,
`RefreshControl`, the live preview, and the `WidgetView`/`WidgetHost` render dispatch — wired **inside the new tabs**.

## Related

- [`README.md`](README.md) — the viz umbrella + phasing (this is Phase 1's editor).
- [`panel-model-scope.md`](panel-model-scope.md) — the spine: the cell shape this editor authors and seeds.
- [`chart-types-scope.md`](chart-types-scope.md) (viz picker + per-view options) ·
  [`field-config-scope.md`](field-config-scope.md) (the Field/Thresholds/Mappings tabs) ·
  [`transformations-scope.md`](transformations-scope.md) (the Transform tab) ·
  [`datasource-binding-scope.md`](datasource-binding-scope.md) (the Query tab's datasource dropdown) ·
  [`import-export-scope.md`](import-export-scope.md) (the import/export UI, a sibling surface).
- [`../widget-builder-scope.md`](../widget-builder-scope.md) — the v2 builder this editor supersedes (reusing its pieces).
- [`../../ui-standards-scope.md`](../../ui-standards-scope.md) — shadcn-first primitives + the canonical look.
- [`../../routing-scope.md`](../../routing-scope.md) — the `?edit=<cellId>` deep-link option.
- [`../../../prefs/user-prefs-scope.md`](../../../prefs/user-prefs-scope.md) — units/numbers/dates the Field tab renders through.
- [`../../../testing/testing-scope.md`](../../../testing/testing-scope.md) — the real-gateway test doctrine.
- [`../../../../public/frontend/dashboard.md`](../../../../public/frontend/dashboard.md) — where this promotes (and the
  "add==edit, one field-code path" claim this scope finally makes true).
- The Grafana reference clone at `/tmp/grafana` — `public/app/features/dashboard-scene/panel-edit` (the editor
  layout we adopt: preview + viz picker + options pane + option search).
- README **§6.13** (UI delivery), **§3** (rules 1/5/6/7).
