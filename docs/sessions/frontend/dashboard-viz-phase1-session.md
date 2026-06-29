# Dashboard viz — Grafana-compatible visualization, Phase 1 (session)

- Date: 2026-06-29
- Scope: ../../scope/frontend/dashboard/viz/README.md (+ panel-model / field-config / chart-types / panel-editor sub-scopes)
- Stage: S9+ collaboration UI — additive over the shipped v2 widget contract
- Status: done

## Goal

Ship Phase 1 of the viz slice — `timeseries` end to end on the Grafana-aligned panel model: the
additive v3 cell shape, the `timeseries` renderer with the full Grafana option surface, the fieldConfig
render path through the one user-prefs bridge (with the documented fallback until `lb-prefs` ships), and
the redesigned one-editor (add≡edit) that fixes the reported "edit loses my SQL options" bug. Exit gate:
a saved timeseries panel reopens in the editor with **every** option identical; v1/v2 cells still load and
round-trip; all formatting flows through one bridge file; mandatory deny + workspace-isolation green.

## What changed

### Backend (Rust) — additive v3 cell shape (the spine), opaque where the UI owns the semantics
- `rust/crates/host/src/dashboard/model.rs` — `Cell` gains serde-default v3 fields: `description`,
  `sources: Vec<Target>` (the new `Target` struct), `transformations: Vec<Value>` (opaque — shape only,
  per invariant B no host execution in Phase 1), `field_config: Value` (opaque — the UI owns the typed
  shape + the prefs bridge), `plugin_version`. `Dashboard` gains `schema_version` (our doc version,
  distinct from `Cell.v`). `SCHEMA_VERSION = 3` pinned at save.
- `rust/crates/host/src/dashboard/bounds.rs` (new) — the record caps (≤32 transforms, ≤64 overrides/
  mappings/threshold-steps); the **host is the boundary** — `save` rejects an over-cap record.
- `save.rs` wires the bounds check + pins `schema_version`. `mod.rs`/`lib.rs` re-export `Target`/caps
  (aliased `CellTarget`/`DASHBOARD_MAX_*` at the crate root to avoid the workflow `Target` collision).

### Frontend — the spine types + the fieldConfig render path (one responsibility per file)
- `ui/src/lib/dashboard/fieldconfig.types.ts` (new) — `FieldConfig`/`FieldOptions`/`Matcher`/
  `ValueMapping`/`ThresholdsConfig`/`FieldColor` (Grafana names verbatim).
- `ui/src/lib/dashboard/dashboard.types.ts` — `View` gains Grafana panel ids; `Target`/`DataSourceRef`/
  `Transformation`; v3 `Cell` fields; `Dashboard.schemaVersion`; `canonicalView` alias map (`chart`→
  `timeseries`); `cellSources`/`cellPrimaryTarget`/`cellFieldConfig` adapters; `cellView` canonicalizes.
- `ui/src/features/dashboard/fieldconfig/` (new): `units.ts` (Grafana-unit→dimension table + picker),
  `format.ts` (**THE** bridge — the only formatter; documented fallback, `viaPrefs` swap flag),
  `thresholds.ts`, `color.ts`, `mappings.ts`, `matchers.ts`, `resolve.ts` (defaults+overrides), `caps.ts`.

### Frontend — the timeseries panel
- `ui/src/features/dashboard/views/timeseries/` (new): `options.ts` (legend/tooltip, Grafana defaults),
  `custom.ts` (drawStyle/lineWidth/fillOpacity — `fieldConfig.custom`), `Legend.tsx`, `TimeseriesView.tsx`.
- `widgets/recharts.tsx` gains `TimeseriesChart` (draw style + threshold color + tooltip).
- `views/WidgetView.tsx` routes `timeseries` (and the `chart` alias) → `TimeseriesView`; `cellTools`
  folds v3 `sources[]` into the bridge leash.
- `builder/usePanelData.ts` (new) — **THE one data hook** (invariant A): Phase 1 reads the primary
  target over the v2 bridge; Phase 3 swaps its body to `viz.query` in one file. No scattered fetches.

### Frontend — the one editor (add ≡ edit)
- `ui/src/features/dashboard/editor/` (new): `cellEditorState.ts` (**THE** pure (de)serializer —
  `editorStateToCell(cellToEditorState(c)) ≡ c`), `defaultCell.ts`, `viewOptions.ts`, `PanelEditor.tsx`
  (Sheet shell + tabs + preview), `VizPicker.tsx`, `OptionsSearch.tsx`, `PreviewPane.tsx`, `Tabs.tsx`,
  `AddPanel.tsx`, `EditCellButton.tsx`, and `tabs/` (Query/Transform/PanelOptions/Field/Overrides +
  `ThresholdsEditor.tsx`). Reuses (not rebuilds) the source picker, SQL Builder⇄Code editor, RefreshControl,
  live preview, `WidgetView`/`WidgetHost`.
- `DashboardView.tsx` mounts `AddPanel`; `Grid.tsx` mounts `EditCellButton` — both open the ONE
  `PanelEditor`. Retired `builder/CellSettings.tsx` (deleted) from the dashboard path; `WidgetBuilder.tsx`
  is kept only as the home of the reused `seedEntryId` + its own v2 tests.

## Decisions & alternatives

- **Keep the view RAW in editor state; canonicalize only at render/display.** `cellToEditorState` stores
  `cell.view` unchanged so `editorStateToCell(cellToEditorState(c)) ≡ c` is byte-identity for a v2
  `chart` cell (it stays `chart`, rendered via the alias). The picker/per-view tabs compare
  `canonicalView(state.view)`. *Rejected:* canonicalizing in the (de)serializer — it silently rewrote a
  v2 cell's `view` and broke the round-trip identity the whole slice hangs on.
- **`fieldConfig`/`transformations` are opaque `Value` on the host.** The UI owns the typed shape + the
  prefs render bridge; the host stores + bounds the JSON. Honors "no options semantics in the host" and
  invariant B (no host transform execution in Phase 1; `lb-viz`/`viz.query` is Phase 3). *Rejected:*
  typed Rust mirror — premature duplication of a shape the backend doesn't yet interpret.
- **One data hook (`usePanelData`) wrapping `useSource`.** Invariant A — the Phase-3 `viz.query` swap is
  one file. The renderer/preview never call `bridge.call` directly.
- **The store drops an explicit JSON `null`.** A threshold base step `{value:null}` round-trips as
  `{color}` (key absent). The render bridge (`thresholds.ts`) treats absent === -∞, and the tests assert
  field-level fidelity (colors + the non-base value), not byte-equality on the dropped null.
- **shadcn-first for the editor.** Buttons use the `<Button>` primitive; native `<select>`/checkbox keep
  per-line justified `no-restricted-syntax` disables (the shipped WidgetBuilder precedent — no shadcn
  Select/Checkbox primitive yet; generating them is the documented `dashboard.md` follow-up). The title
  input uses the `<Input>` primitive.

## Tests

Real gateway/store, seeded real rows, no mocks (CLAUDE §9). Mandatory categories: capability-deny
(host save-deny backstop), workspace-isolation, backward-compat, the format-bridge "no stored string".

- **Rust** `cargo test -p lb-host` — all green, incl. new `dashboard_test`:
  `v3_cell_round_trips` (every v3 field through save/get; schemaVersion pinned),
  `v1_and_v2_cells_still_load_after_v3` (byte-identity, no v3 injection),
  `over_cap_v3_record_is_rejected` (the host bounds). Gateway: `dashboard_routes_test` 6/6.
- **UI unit** `pnpm test` — 138 green, incl. `editor/cellEditorState.test.ts` (round-trip identity for
  v1/v2/v3/default cells + key/geometry preservation), `fieldconfig/format.test.ts` (bridge fallback,
  unmapped-unit degrade, `viaPrefs` swap-point, non-numeric honesty), `fieldconfig/fieldconfig.test.ts`
  (thresholds/mappings/resolve).
- **UI gateway** `pnpm test:gateway` — 151 pass + 1 pre-existing SystemView flake (passes isolated 9/9).
  New `editor/panelEditor.gateway.test.tsx` (6): **ADD ≡ EDIT parity headline** (SQL Builder + fieldConfig
  unit/decimals/threshold + per-viz options + transform config → save → reopen → every option identical),
  backward-compat (v1 + v2 idempotent + semantics intact), live preview over real seeded rows + honest
  deny, edit-cap host backstop, workspace isolation. `DashboardView.gateway.test.tsx` updated to the new
  editor flow (Add panel → editor → save; ⚙ edit reopens the same editor; `timeseries` labels; v2 `chart`
  cell renders via the alias).

### Green output (key runs)

```
cargo test -p lb-host --test dashboard_test
test result: ok. 10 passed; 0 failed
cargo test -p lb-role-gateway --test dashboard_routes_test
test result: ok. 6 passed; 0 failed
ui$ pnpm test            → Test Files 21 passed (21) | Tests 138 passed (138)
ui$ pnpm test:gateway editor/panelEditor → 6 passed (6)
ui$ pnpm test:gateway DashboardView      → 6 passed (6)
ui$ pnpm test:gateway    → 151 passed, 1 pre-existing SystemView flake (9/9 isolated)
ui$ pnpm exec tsc --noEmit → clean
```

## Debugging

None opened — no defect required a `debugging/` entry. The store-null-normalization and the
view-canonicalization-vs-round-trip tension were resolved in design (above), not as bugs.

## Public / scope updates

- Promoted to `docs/public/frontend/dashboard.md` — new "Grafana-compatible visualization — Phase 1"
  section.
- Marked Phase 1 shipped in the viz README phasing + the four sub-scope status lines.
- `STATUS.md` — new shipped slice row.

## Dead ends / surprises

- The host serializes **all** serde-default fields, so a cell fetched from the gateway is fully
  materialized (empty v3 defaults present). Byte-identity with a hand-built minimal cell only holds for
  the *pure* (de)serializer (unit test); the gateway tests assert **idempotency + semantic** fidelity.
- **Pre-existing blocker (not this slice):** an untracked in-flight `role/gateway/src/routes/prefs.rs`
  (the user's concurrent `lb-prefs` work) imports `lb_prefs`, which isn't a gateway dependency — so
  `cargo test --workspace` fails to compile the gateway *lib unit-test* target. The gateway *binary* and
  *integration tests* build fine (dashboard routes 6/6; UI gateway suite spawns the real node). Flagged
  for the prefs author; out of scope here.
- Two pre-existing lint errors in `vars/VariableEditor.tsx` (7) + `studio/StudioView.tsx` (1) are
  unmigrated non-LEGACY files (no diff from HEAD) — pre-existing tech debt, not introduced here.

## Follow-ups

- Phase 2: the rest of the standard chart set on this spine.
- Phase 3: `viz.query` + `lb-viz` (swap `usePanelData`'s body), multi-datasource targets, the Transform
  tab's real pipeline.
- `lb-prefs`: when it ships, swap `fieldconfig/format.ts`'s fallback for the real `format.*` call — no
  schema change, no re-save (the `viaPrefs` flag is the swap-point guardrail).
- Generate shadcn `Select`/`Checkbox` primitives so the editor drops the justified native-control disables.
- STATUS.md updated: yes.
