# `scope/frontend/query-builder/` — the visual query builder subtopic

The 10x of the shipped visual SQL builder, taking the good parts of **Tabularis** (cloned at
`/tmp/tabularis`), plus a standalone **Query workbench** surface. Start with the umbrella.

| Doc | What it is |
|---|---|
| [`query-builder-10x-scope.md`](query-builder-10x-scope.md) | **Umbrella.** The thesis (port the interaction, not the code), the three slices, the cross-cutting decisions (model-as-truth, extend-emitters, CodeMirror-not-Monaco, one-component/three-homes), and the saved-query seam left for the user. **Read first.** |
| [`visual-canvas-builder-scope.md`](visual-canvas-builder-scope.md) | Slice 1 — extend `SqlBuilderQuery` with joins/HAVING/aliases/multi-sort/OR; a `@xyflow/react` canvas as a *view* over the typed model; extend both dialect emitters. No backend. |
| [`sql-editor-10x-scope.md`](sql-editor-10x-scope.md) | Slice 2 — schema-aware completion via `@codemirror/lang-sql`, a ported dialect-aware statement splitter, and a `sql-formatter` Format button. Stay on CodeMirror. No backend. |
| [`query-workbench-view-scope.md`](query-workbench-view-scope.md) | Slice 3 — a `/t/$ws/query` standalone view (like Flows/Rules) that also opens as a Data Studio pane; runs real queries; carries the mandatory capability-deny + workspace-isolation gateway tests. No backend. |
| [`tabularis-harvest.md`](tabularis-harvest.md) | **Advisory.** Everything else in Tabularis worth taking (notebook, visual EXPLAIN, rich grid, …) with a take-now / scope-later / skip verdict + reason each. Not a scope. |

**Shipped precedent this builds on:** `../query-builder-common-scope.md` (the one-builder/N-emitter seam,
already shipped 2026-07-06). **Where it lands:** `../data-studio-10x-scope.md` (the Data Studio surface) and
`docs/prompts/data-studio/README.md` (the standalone-view registration map).

**Golden constraints (from CLAUDE.md, restated so no slice drifts):** rule 2 (external engines are
federation *sources*, never new datastores) · rule 5 (every read/run is capability-gated; deny degrades
honestly) · rule 6 (workspace wall; no workspace argument) · rule 8 (one responsibility per file) · rule 9
(no mocks — real gateway + real SQLite demo datasource) · rule 10 (dialect from `kind`, never a datasource
name; the canvas/editor never branch on an id).
</content>
