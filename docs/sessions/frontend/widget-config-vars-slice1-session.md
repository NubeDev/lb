# Frontend dashboard — widget settings/config (edit, not re-add) — Slice 1 (session)

- Date: 2026-06-28
- Scope: ../../scope/frontend/dashboard/widget-config-vars-scope.md (Slice 1)
- Stage: post-S8, building on the shipped widget-builder v2 + widget-palette (STATUS.md "Slices in flight")
- Status: done
- Public: ../../public/frontend/dashboard.md → "Widget settings (edit a cell)"
- Tests: rust/crates/host/tests/dashboard_test.rs (`cell_title_round_trips`),
  ui/src/features/dashboard/builder/widgetBuilder.test.ts (unit: `cellLabel`, `seedEntryId`),
  ui/src/features/dashboard/DashboardView.gateway.test.tsx (real-gateway ⚙ round-trip)

## Goal

Editing a widget today is "delete it and add a new one." Slice 1 adds a per-cell **settings/config**
surface: a `title`, and a ⚙ button (edit mode + edit cap) that opens a drawer reusing the WidgetBuilder
source/view/option fields in an **edit-existing-cell** mode — seed from the cell, write back, persist the
whole dashboard via the existing `saveCells`/`dashboard.save` (no new verb).

Acceptance (scope testing plan): the cell `title` round-trips through `dashboard.save`/`get`; the ⚙
affordance is gated on `mcp:dashboard.save:call` (reusing the widget-palette gate); add → ⚙ → rename +
change view/options → save → reload re-renders with the edits; the header renders the title (fallback to a
derived label).

## What changed

Backend (additive serde, no new verb):

- `rust/crates/host/src/dashboard/model.rs` — `Cell` gains `title: String` (`#[serde(default)]`). A
  pre-title cell deserializes unchanged; `dashboard.save`/`get` already round-trip arbitrary serde fields.

Frontend (one responsibility per file, FILE-LAYOUT):

- `ui/src/lib/dashboard/dashboard.types.ts` — `Cell.title?: string`; a new `cellLabel(cell)` helper
  (title → source tool → action tool → view/widget_type), so the header always reads something honest.
- `ui/src/features/dashboard/views/WidgetView.tsx` — default the per-view header label to `cellLabel(cell)`
  so every built-in view shows the configured title (the views already render `WidgetHeader label`).
- `ui/src/features/dashboard/builder/WidgetBuilder.tsx` — additive **edit mode**: a `title` field
  (always), an optional `seed: Cell` (seeds source/view/options/title from the cell once the picker
  entries load), an `onSave` (rebuilds the cell keeping its `i`/geometry), and a `bare` flag (no panel
  chrome). The exported `seedEntryId(cell, entries)` maps an existing cell back to its picker entry
  (packaged tile by view key, SQL by `store.query`, else read/action tool + series arg). Add stays
  `onAdd`; edit calls `onSave`.
- `ui/src/features/dashboard/builder/CellSettings.tsx` (new) — the Sheet drawer hosting the builder in
  edit mode + a per-cell ⚙ `CellSettingsButton` that owns its open state.
- `ui/src/features/dashboard/Grid.tsx` — render the ⚙ button when `editable && canEdit` (mirrors the
  remove button's gate); a new `onEditCell` write-back.
- `ui/src/features/dashboard/DashboardView.tsx` — thread `canEdit` + an `onEditCell` that splices the
  edited cell into the layout by `i` and persists via `saveCells`.

## Decisions

- **Reuse WidgetBuilder, don't fork it.** The scope says "reuse the builder's source/view/option fields in
  an edit-existing mode." Rather than a parallel editor (drift risk), WidgetBuilder gained `seed`/`onSave`/
  `bare` — the exact same fields, in a drawer. This keeps one authoring surface and one set of field code.
- **Title is a derived-label fallback, not a required field.** `cellLabel` never returns empty, so an
  untitled cell still has an honest header (its source tool / view). The backend stores `""` for untitled.
- **Gate = the widget-palette edit gate.** The ⚙ shows only with `mcp:dashboard.save:call` (sourced from
  the routing context's `caps`, the same place the add-builder is gated); the host re-checks on save.

## Tests + green output

Backend — `cargo test -p lb-host --test dashboard_test`:

```
running 6 tests
test cell_title_round_trips ... ok
test each_verb_is_denied_without_its_cap ... ok
test crud_round_trip ... ok
test workspace_isolation ... ok
test team_shared_member_reads_non_member_denied ... ok
test seed_writes_real_tagged_series ... ok
test result: ok. 6 passed; 0 failed
```

Frontend unit — `vitest run widgetBuilder.test.ts`: **19 passed** (6 new Slice-1 cases: `cellLabel` title/
derived/non-empty, `seedEntryId` for series/ext-tile/SQL).

Frontend real-gateway — `vitest run --config vitest.gateway.config.ts DashboardView.gateway.test.tsx`:
**4 passed** (the new `Slice 1 — ⚙ settings: add → rename + change view → save → reload re-renders with
edits` drives the real node: add a chart bound to a seeded series → ⚙ → rename "Web01 CPU" + switch view
to stat → save → the title shows + persists across reload + the stat value renders). The
`react-grid-layout` "onDragEnd called before onDragStart" lines are pre-existing jsdom teardown noise from
the grid (present in the prior suite), not test failures — the run reports 4 passed.

## Mandatory categories

- **Capability deny:** the ⚙/edit affordance is gated on `mcp:dashboard.save:call` (UI), and the host
  re-checks `dashboard.save` server-side regardless — the per-verb deny is already covered by the shipped
  `each_verb_is_denied_without_its_cap` (save denied without the cap). No new verb, so no new deny surface.
- **Workspace isolation:** unchanged — the dashboard record (incl. cell titles) is workspace-scoped; the
  shipped `workspace_isolation` test covers it. Editing persists through the same walled `dashboard.save`.

## Follow-ups

None for Slice 1. Next: the shared `vars` library, then Slice 2 (variables).
