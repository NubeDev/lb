# Session: "run the sql" in the panel editor

## Ask

In the new chart/widget Edit panel, a federation/timescale datasource SQL query
(`select * from sites`) was typed but the live preview stayed on **"no data yet"** —
the user wanted the SQL to actually run.

## Finding

The preview already runs the real query: `PreviewPane` → `WidgetView` → `usePanelData`
→ `useVizQuery` issues a debounced `viz.query(panel)` over the bridge (panel-editor
scope: "live preview is the real thing"). So the *mechanism* to run SQL exists.

The bug was in the (de)serializer `editorStateToCell`
([cellEditorState.ts](../../../ui/src/features/dashboard/editor/cellEditorState.ts)).
A freshly-added, target-less cell deserializes with `carry.targetRepr === "none"`.
Authoring a federation target in the Query tab lands it in `state.targets`, but the
serializer only wrote `cell.sources`/`cell.source` for the `"sources"`/`"source"`
reprs — the `"none"` branch **silently dropped the authored target**. The draft cell
fed to the preview therefore had no source, so `useVizQuery` saw `!hasTarget` and the
panel showed "no data yet". Save would also have persisted an empty panel.

## Fix

When `targetRepr === "none"` but the author has supplied a target with a real `tool`,
promote to the v3 `sources[]` shape. A still-empty (no-tool) target stays absent, so a
genuinely blank panel still round-trips clean and a v1/v2 cell is unaffected.

With the target serialized, the debounced `viz.query` runs the SQL and the preview
populates.

## Run button

The user explicitly wanted a **Run** affordance, not just the silent auto-preview. Added
a `Run` button (▶) to the federation SQL block in
[QueryTab.tsx](../../../ui/src/features/dashboard/editor/tabs/QueryTab.tsx), plus
Cmd/Ctrl+Enter in the SQL textarea. `PanelEditor` owns a `runNonce` folded into the
preview's `refreshKey`; pressing Run bumps it, forcing `useVizQuery` to re-fire even when
the spec is byte-identical (re-running the same SQL). The button disables until a source
and non-empty SQL are present.

## Test

Added a regression to
[cellEditorState.test.ts](../../../ui/src/features/dashboard/editor/cellEditorState.test.ts):
a target-less cell authored with a `federation.query {source, sql}` target serializes a
queryable `sources[]`; an empty (no-tool) target produces no spurious source.

`cd ui && pnpm test` → 22 files, 148 tests passing (+1).
