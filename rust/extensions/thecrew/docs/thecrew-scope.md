# thecrew scope — the graphics-canvas UI/UX test bed

Status: **done as a playground** (phases 1–3 built, see
`docs/sessions/frontend/thecrew-session.md`). The package has moved to
`rust/extensions/thecrew/` (app code under `ui/`) and the next ask is the lift into
a real LB extension — see **`thecrew-extension-scope.md`** (this folder). This doc
is kept as the playground's contract and the visual/builder bar.

This package began as a **playground**, deliberately outside the node/gateway stack;
what it proves migrates into the extension
(`docs/scope/frontend/graphics-canvas-scope.md`), and what it disproves dies cheaply here.

We committed to one engine (three.js via `@react-three/fiber`) and a 100%-extension
posture for the graphics canvas. Before building any of the platform plumbing, we
answer the two questions that actually decide whether this feature is worth shipping:
**does it look amazing** and **does building a page feel amazing**. `thecrew` is a
standalone React app — palette on the left, canvas in the middle, property rail on the
right — in which a user builds an AHU from duct/equipment symbols and draws a small
floor plan. 100% of the effort goes to look and feel. Zero goes to backend, auth,
persistence infrastructure, or federation.

## Goals

- **Prove the look** (`look-scope.md`): a flat, orthographic plant graphic rendered in
  three.js that a Niagara user would screenshot and send to a colleague. Then the same
  scene tilted into 3D, and it still holds up.
- **Prove the builder** (`builder-ux-scope.md`): palette → drag → snap → connect →
  tune-in-rail, measured by the **60-second AHU** benchmark.
- **Prove the symbols** (`symbols-scope.md`): a starter HVAC set (ducts, fan, damper,
  filter, coil, casing) + floor-plan set (walls, rooms, doors, labels) with one design
  language.
- **Stay portable.** Everything under `src/scene/`, `src/canvas/`, `src/theme/`, and
  `src/editor/` is written to lift into the framework extension unchanged (see the
  reuse contract below).

## Non-goals

- No lb node, gateway, capabilities, workspaces, or MCP — nothing in `src/` imports
  from `ui/` or `rust/`.
- No AI drawing yet. The scene *document* is AI-ready (flat id map, validated); the
  agent loop lives in the framework phase.
- No symbol-pack loading; symbols are hardcoded React components here. The pack format
  is the extension scope's problem.
- No multi-user, no undo-journal integration (local undo stack only), no routing.
- No pixel-perfect mobile; desktop-first playground.

## The reuse contract (what makes this liftable)

1. **The scene document is the framework's schema.** `src/scene/scene.types.ts`
   implements the shape map from the graphics-canvas scope (`v`, `camera`, `shapes`,
   `t`, `props`, `bind`). If the playground needs a schema change, that's a finding to
   push back into the extension scope, not a local fork.
2. **One data seam.** Shapes never fetch; they receive resolved values. The only source
   is `src/data/value-source.ts` (an interface). The playground implementation is
   `src/data/simulator.ts` — **the one declared fake in this package** (allowed: there
   is no node here at all; it is clearly named, behind one interface, and is exactly
   what the host bridge replaces). No other fake may be added.
3. **Theme by tokens.** All color/material decisions flow through `src/theme/` —
   nothing hardcodes a hex in a shape component — so the extension can bind them to the
   shell's token system later.
4. **No app-shell bleed.** `src/editor/` and `src/canvas/` take props/stores, never
   reach into `App.tsx` layout specifics.
5. **FILE-LAYOUT applies** — one responsibility per file, ≤400 lines hard; the layout
   below is the contract.

## File layout

```
rust/extensions/thecrew/          # (playground-era layout; src/ now lives at ui/src/)
├── docs/                      # these scopes
├── index.html                 # Vite entry
├── package.json               # @nube/thecrew (private playground app)
├── vite.config.ts / tsconfig.json
└── src/
    ├── main.tsx               # bootstrap
    ├── App.tsx                # layout shell: Toolbar / Palette / Canvas / PropertyRail
    ├── styles.css             # Tailwind v4 + token variables (DOM chrome only)
    ├── theme/
    │   ├── tokens.ts          # the design tokens (color/space/motion) — the look's source of truth
    │   └── materials.ts       # tokens → three.js materials (one place; shapes never `new Material`)
    ├── scene/
    │   ├── scene.types.ts     # THE scene document schema (framework contract)
    │   ├── validate.ts        # total validation + normalization (unknown type → placeholder)
    │   ├── defaults.ts        # per-type default transforms/props (what the palette drops)
    │   └── demo/
    │       ├── ahu-demo.ts    # seeded AHU-1 scene (the look-scope hero shot)
    │       └── floorplan-demo.ts
    ├── state/
    │   └── scene-store.ts     # zustand store: doc + selection + camera mode + undo stack
    ├── data/
    │   ├── value-source.ts    # the seam: subscribe(channel) → value stream (interface)
    │   └── simulator.ts       # THE declared fake: plausible plant values (fan rpm, temps, status)
    ├── canvas/
    │   ├── SceneCanvas.tsx    # <Canvas> + lighting + postprocessing rig
    │   ├── CameraRig.tsx      # ortho-top (flat) ↔ persp, spring transition
    │   ├── ShapeNode.tsx      # type → component dispatch + selection halo + placeholder box
    │   └── shapes/            # one symbol per file (see symbols-scope.md)
    │       ├── shape-props.ts # the shared ShapeComponentProps contract
    │       ├── Duct.tsx  Fan.tsx  Damper.tsx  Filter.tsx  Coil.tsx  AhuCasing.tsx
    │       ├── Wall.tsx  Room.tsx  Door.tsx
    │       └── Label.tsx
    └── editor/
        ├── Toolbar.tsx        # camera toggle, snap toggle, undo/redo, demo switcher
        ├── Palette.tsx        # the symbol palette (builder-ux-scope §palette)
        ├── PropertyRail.tsx   # selected-shape schema-driven editor
        ├── use-selection.ts   # click/box select state + raycast plumbing
        ├── use-drag-place.ts  # palette → canvas drag, ghost preview, drop
        ├── use-snap.ts        # grid + anchor snapping
        └── use-undo.ts        # local undo/redo over the store
```

## Testing plan

This package is a **look playground**, so the bar is judged, not only asserted — but
what can be asserted, is:

- **Unit (`vitest`):** `scene/validate.ts` (unknown type, missing transform, bad bind),
  `defaults.ts`, `editor/use-snap.ts` math, `use-undo.ts` push/undo/redo, simulator
  channel determinism.
- **Render (r3f test renderer, no WebGL pixels in CI):** the scene graph built from
  `ahu-demo.ts` contains the expected nodes; unknown type renders the placeholder;
  selection halo appears on select.
- **The screenshot test (manual, mandatory):** every phase ends with screenshots of
  both demos checked into `docs/shots/` — the look-scope review artifact.
- **The 60-second AHU (manual, mandatory):** builder-ux-scope's benchmark, run and
  honestly reported at each phase end.

No fake beyond `simulator.ts` (declared above). No `*.fake.ts` files.

## Phases

1. **The hero shot.** Scene schema + validate + `ahu-demo` seeded doc rendered
   read-only in flat mode, with the full look pipeline (lighting, materials, glow,
   animated duct flow). *Exit: the screenshot makes people ask "what is that?"*
2. **The builder.** Palette, drag-place with ghost + snap, selection + gizmo, property
   rail, undo. *Exit: the 60-second AHU passes.*
3. **Floor plan.** Wall/room/door symbols, wall-chain drawing tool, the floorplan demo.
   *Exit: a recognizable office floor drawn from scratch in ~3 minutes.*
4. **The tilt.** Camera toggle to perspective; extruded walls, casing depth, 3D still
   looks intentional (not "2D assets standing up"). *Exit: side-by-side shot flat/3D.*
5. **Lift report.** A short addendum here: what moves into the extension as-is, what
   needs rework, what the playground disproved.

## Definition of done

- Both demo scenes build, look, and feel per the three sibling scopes.
- Phase screenshots in `docs/shots/`; benchmark results recorded.
- The reuse contract held (grep-clean: no `ui/`/`rust/` imports, one fake, tokens only).
- The lift report exists and the graphics-canvas extension scope is updated with the
  findings.

## Related

- `docs/scope/frontend/graphics-canvas-scope.md` (repo root) — the framework feature
  this de-risks; its schema is authoritative.
- Siblings: `look-scope.md`, `builder-ux-scope.md`, `symbols-scope.md`.
- `docs/FILE-LAYOUT.md`, `docs/scope/testing/testing-scope.md` §0 (the one-fake rule).
