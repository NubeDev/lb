# thecrew scope — the builder UX

Status: scope (the ask). How building a page must *feel*. The look gets people to stop;
the builder decides whether they stay.

## The benchmark: the 60-second AHU

A first-time user, told only "build an air handler", produces in ≤60 seconds: casing →
outside-air damper → filter → coil → fan → connected duct run → fan bound to a
simulator channel. If any step makes them hunt, the UX failed — not the user. This is
run and recorded at the end of every phase (`thecrew-scope.md` testing plan).

## The loop: palette → place → connect → tune

**Palette (left, `Palette.tsx`)**
- Two categories for now: **HVAC** and **Floor plan** (from `symbols-scope.md`), search
  box on top, big thumbnails (the real component rendered small — never static PNGs).
- Hover: thumbnail animates (fan spins, damper sweeps) — the palette *demos* each
  symbol.
- Drag out → a translucent **ghost** of the real symbol follows the cursor on-canvas,
  already snapped; drop places it with defaults from `scene/defaults.ts`. Click-to-arm
  + click-to-place also works (accessibility + trackpads).

**Place & snap (`use-drag-place.ts`, `use-snap.ts`)**
- Always-on grid snap (toolbar-toggleable), with **anchor magnetism**: near a duct
  anchor, the ghost jumps to it and shows a connect affordance. Placing "onto" an
  anchor places *and* connects in one gesture.
- Ducts draw like a polyline tool: click anchor → click, click, double-click to end;
  elbows are inserted automatically at corners. Walls draw the same way (chain tool) —
  one drawing gesture to learn, two symbol families.

**Select & transform (`use-selection.ts`, gizmo in `ShapeNode.tsx`)**
- Click = select (cyan halo, rail populates). Shift-click adds; drag on empty canvas =
  box select. Esc clears.
- Move = just drag the shape (no mode switch). Rotate/scale via drei
  `TransformControls` on the selection, flat mode constrained to the plane (no
  accidental tilt — flat mode locks orbit entirely).
- Delete/duplicate: `Del`, `Cmd/Ctrl-D`; arrows nudge by one grid step.

**Tune (right, `PropertyRail.tsx`)**
- Schema-driven from the symbol's prop schema: label, size, medium, then **bindings** —
  a channel picker over the simulator's channels (the seam the framework replaces with
  its real source picker).
- Every change reflects on-canvas immediately; no Apply button. The rail shows the
  live bound value next to each binding — tune-and-watch is the demo moment.

**Undo (`use-undo.ts`)**
- Every completed gesture (place, move-end, connect, delete, prop commit) is one undo
  step. `Cmd/Ctrl-Z` / `Shift-Cmd-Z`. Depth ≥50. No gesture may be un-undoable.

## First-run experience

Never an empty void: the app opens on the **AHU demo scene** with a one-line hint bar
("drag from the palette · double-click to inspect"). The toolbar's demo switcher swaps
to the floor plan or a blank page. Blank page shows a centered ghost-text prompt, not
darkness.

## Keyboard map (v1)

`V` select · `D` duct/wall chain tool · `Esc` cancel/clear · `Del` delete ·
`Cmd-Z`/`Shift-Cmd-Z` undo/redo · `Cmd-D` duplicate · `G` toggle grid snap ·
`Tab` toggle flat/3D · arrows nudge. Shown in a `?` overlay.

## Anti-goals

- Modes that trap (no "you're in duct mode forever"); every tool exits on Esc and on
  completing the gesture.
- Property dialogs/modals — the rail is the only editor surface.
- Toolbars of 30 icons; if a control isn't used in the 60-second AHU or the floor-plan
  benchmark, it doesn't ship in this playground.

## Open questions

- Duct auto-elbow: insert real elbow symbols vs. render the polyline with styled
  corners? (Leaning polyline-with-styled-corners here; real elbows become a symbol-pack
  concern in the framework.)
- Box select in an orthographic three.js scene: screen-space rect over projected
  bounds is the plan — validate perf on ~200 shapes.
- Does the rail need per-shape "advanced" collapse in v1, or are symbol schemas small
  enough? (Leaning: keep schemas ≤8 props and skip it.)

## Related

- `thecrew-scope.md` (benchmark cadence), `symbols-scope.md` (anchors + prop schemas),
  `look-scope.md` (cursors, halo, chrome). Framework home:
  `docs/scope/frontend/graphics-canvas-scope.md` (editor phase inherits this doc).
