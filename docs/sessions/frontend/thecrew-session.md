# Session — thecrew: phases 1–3 build-out (playground)

**Scope:** `packages/thecrew/docs/thecrew-scope.md` (+ look / builder-ux / symbols siblings)
**Date:** 2026-07-02
**Outcome:** the skeleton became a working app — both demo scenes render through the full
look pipeline, the builder loop (palette → place → connect → tune → undo) is wired,
and the package is green: `tsc --noEmit` clean, **37/37 vitest** passing.

## What was built

Foundation (contracts first, everything else conforms):

- `theme/tokens.ts` — extended with per-medium accents (air/chw/hw — look-scope open
  question decided: per-medium, capped at three), duct body + text colors.
- `theme/materials.ts` — the one materials factory. Selective bloom implemented as a
  *convention*: Bloom runs at `luminanceThreshold: 1`; only status emissives
  (`emissiveIntensity 2`) and the selection halo cross it. Fault pulses at 0.5 Hz via a
  per-frame `updateMaterials()` tick. Per-duct chevron flow textures (`ductFlowMaterial`).
- `scene/validate.ts` — total validation: any input → renderable doc + teaching issues
  (unknown type is *legal* and renders the placeholder).
- `scene/defaults.ts` — canonical prop names per type (the demos/rail/shapes contract).
- `data/simulator.ts` — **the one declared fake**: every value is
  `sampleChannel(channel, tSec)`, pure and deterministic; the live source is one ticker
  over it. 20 channels (ahu1.\*, zone.101–106.\*).
- `data/use-values.ts` — the React face of the ValueSource seam (context + hooks);
  the framework's bridge swaps the provider, shapes never know.
- `state/scene-store.ts` — zustand; snapshot undo (depth 64), every completed gesture is
  one step; demo switcher; camera mode lives in the doc.
- `editor/use-snap.ts` — pure snap math; anchor magnetism *wins over* grid.
- `canvas/ShapeNode.tsx` — registry dispatch + placeholder + halo + hover anchors +
  per-shape `Suspense` (a slow SDF font must never blank a sibling shape).

Symbols (all ten, per the symbols-scope contract tables): hvac duct/fan/damper/filter/
coil/casing, plan wall/room/door/label — real shallow-3D geometry (extruded, so the
phase-4 tilt is free), all color through materials/tokens, animated: chevron flow at
bound rpm, spinning fans, sweeping dampers, breathing room-temperature tint;
`prefers-reduced-motion` freezes all of it.

Look pipeline: `SceneCanvas.tsx` (key light + shadows, fading grid, selective Bloom +
subtle vignette; AO skipped — drei ContactShadows is XZ-native and fights the +Z-up
world; look must survive AO-off anyway) and `CameraRig.tsx` (ortho-top with orbit
locked ↔ persp, one ~600 ms damped spring, instant cut under reduced motion).

Builder: glass Toolbar (demo switcher, camera/snap toggles, undo/redo, `?` keymap
overlay — not a modal), Palette with **live r3f thumbnails** (demand-frameloop, animate
on hover), schema-driven PropertyRail with channel picker + live bound values,
`DropPlane` in-canvas placement (ghost + grid/anchor snap + HTML5 drop + the polyline
chain tool for ducts/walls), keyboard per the builder-ux map.

Demos: `ahu-demo.ts` (casing, OA duct → damper → filter → chw coil → SF-1 → SA duct
with elbow; SAT/ΔP live labels) and `floorplan-demo.ts` (400×256 envelope, 6 rooms
bound to zone temps/occupancy, 4 doors, corridor spine).

## How it was built

Foundation written first (single writer for all contracts), then four parallel
sub-agents: HVAC shapes · plan shapes + demo · look pipeline · editor UI. No file
overlap; one cross-agent integration point (`<DropPlane/>` into SceneCanvas) resolved
by a check-then-patch rule given to the editor agent.

## Testing (per scope testing plan)

- 37 vitest tests: validate (7), defaults (4), snap (6), simulator determinism +
  seam contract (6), store gestures/undo-depth/redo-branch (9), r3f render (5:
  registry coverage, both demo scene graphs, placeholder-not-crash, halo-on-select).
- Render tests use `@react-three/test-renderer` + happy-dom (added as devDep) — real
  components, real store, real simulator; no fakes beyond `simulator.ts`.
- Screenshot test: see `packages/thecrew/docs/shots/` (phase-1 artifacts).
- 60-second AHU: **not yet run with a human** — recorded as open; the mechanics
  (arm → ghost → snap → place, chain tool, rail binding picker) are in place.

## Issues hit (debugging log)

1. **drei `Text` suspends the whole shape subtree offline** — troika fetches a CDN
   font index; offline/CI that suspends/aborts. Fix: per-shape `Suspense` in
   ShapeNode (also the right prod behavior) + assertions that don't depend on fonts;
   the teardown fetch-abort is silenced via `dangerouslyIgnoreUnhandledErrors` with a
   comment (assertions all run before it).
2. **`ductFlowMaterial` crashed headless** — happy-dom has no 2D canvas context.
   Fix: null-ctx guard returns an untextured (invisible) but valid material.
3. **Hex leakage in chrome** — App/PropertyRail hardcoded `#0a0e14`/`#101620`; moved
   to `--tc-canvas` / new `--tc-panel-solid` CSS vars.

## Contract check (thecrew-scope §definition-of-done)

- grep-clean: no `ui/`/`rust/` imports ✓ · one fake (`simulator.ts`) ✓ · no hex
  outside `theme/` + `styles.css` ✓ · all files ≤400 lines ✓.

## Open / next

- Phase-1 exit gate: screenshots into `docs/shots/` + the by-eye look review.
- 60-second AHU benchmark with a human (phase 2 exit).
- Box select (screen-space rect) — TODO in `use-selection.ts`.
- Camera spring fov↔ortho framing pop — phase 4 polish (noted in CameraRig header).
- Duct/wall connect is geometric adjacency only (no edge model) — pushed to the
  graphics-canvas extension scope as a finding.
