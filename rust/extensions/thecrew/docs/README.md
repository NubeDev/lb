# thecrew — scope index

`thecrew` began as the **UI/UX test bed** for the graphics canvas
(`docs/scope/frontend/graphics-canvas-scope.md` in the repo root) and proved the look
and the builder feel (phases 1–3 built, 37/37 vitest — see
`docs/sessions/frontend/thecrew-session.md`). It now lives at
`rust/extensions/thecrew/` and the current ask is the **lift into a real LB
extension** (graphics-canvas phases 1–2).

Read in this order:

| Doc | The ask |
|---|---|
| [`thecrew-extension-scope.md`](thecrew-extension-scope.md) | Playground → publishable LB extension (graphics-canvas phases 1–2, **shipped**) — manifest, wasm stub, bridge value source, `assets.*` scene persistence, `[ui]` page + `[[widget]]` cell. |
| [`symbol-packs-scope.md`](symbol-packs-scope.md) | **The current ask** (graphics-canvas phase 4): symbol packs — new symbols as workspace documents in a parametric part spec, authorable at runtime by hand or by AI; one interpreter, teaching validation, zero core additions. |
| [`thecrew-scope.md`](thecrew-scope.md) | The playground's master scope (done): the reuse contract that makes the code liftable, file layout, phases, definition of done. |
| [`look-scope.md`](look-scope.md) | The visual bar. Theme, lighting, materials, motion, glow — what "looks fucking amazing" means, concretely, and the screenshot test that enforces it. |
| [`builder-ux-scope.md`](builder-ux-scope.md) | The builder feel. Palette → place → connect → tune, snapping, gizmos, property rail, keyboard — and the 60-second-AHU benchmark. |
| [`symbols-scope.md`](symbols-scope.md) | The starter symbol sets (HVAC + floor plan): what each symbol is, its props/bindings/anchors, and its flat vs 3D representation. |

House rules still apply here: `FILE-LAYOUT.md` (one responsibility per file) governs
`ui/src/` and `src/`. The playground's one declared fake (the value simulator) was
allowed only while there was no node; it does **not** lift into the extension — the
bridge value source replaces it (`thecrew-extension-scope.md`).
