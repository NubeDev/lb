# thecrew — scope index

`thecrew` is the **UI/UX test bed** for the graphics canvas
(`docs/scope/frontend/graphics-canvas-scope.md` in the repo root): a standalone
playground whose only job is to prove **the look** and **the builder feel** — build an
AHU from a duct palette, draw a floor plan, and have both look stop-and-stare good —
before any of it touches the framework.

Read in this order:

| Doc | The ask |
|---|---|
| [`thecrew-scope.md`](thecrew-scope.md) | The master scope: what this playground is, the reuse contract that lets the code move into the framework later, file layout, phases, definition of done. |
| [`look-scope.md`](look-scope.md) | The visual bar. Theme, lighting, materials, motion, glow — what "looks fucking amazing" means, concretely, and the screenshot test that enforces it. |
| [`builder-ux-scope.md`](builder-ux-scope.md) | The builder feel. Palette → place → connect → tune, snapping, gizmos, property rail, keyboard — and the 60-second-AHU benchmark. |
| [`symbols-scope.md`](symbols-scope.md) | The starter symbol sets (HVAC + floor plan): what each symbol is, its props/bindings/anchors, and its flat vs 3D representation. |

House rules still apply here: `FILE-LAYOUT.md` (one responsibility per file) governs
`src/`, and the one fake in this package (the value simulator) is declared loudly in
`thecrew-scope.md` — it is the seam the framework's bridge replaces.
