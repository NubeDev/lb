# thecrew scope — the starter symbols

Status: scope (the ask). The two symbol families the playground ships, their design
language, and each symbol's contract (props · bindings · anchors · flat/3D form). In
the framework these become the first symbol pack; here they are hand-written React
components under `src/canvas/shapes/` — one file per symbol.

## Design language (applies to every symbol)

- **Flat, geometric, confident.** Simple extruded forms read top-down as clean 2D
  glyphs and tilt into credible 3D — one geometry, two cameras, per the one-engine
  rule. No skeuomorphic chrome.
- **Status is a slot, not a style.** Every powered symbol exposes the same status
  states (running/stopped/fault/override) rendered via `theme/materials.ts` emissives —
  a symbol never picks its own colors.
- **Anchors are the contract.** Each symbol declares named anchor points (position +
  direction) — where ducts/walls connect and where snap magnetism acts
  (`builder-ux-scope.md`). Anchors render as subtle dots on hover/drag only.
- **Every symbol registers** `type`, prop schema (≤8 props), binding slots, defaults
  (`scene/defaults.ts`), and both representations. The shared component contract is
  `shapes/shape-props.ts`.

## Family 1 — HVAC (`hvac.*`)

| Symbol | File | Props (beyond label) | Bindings | Anchors | Flat form / 3D form |
|---|---|---|---|---|---|
| Duct run | `Duct.tsx` | path points, width, medium | `flow` (drives chevron speed) | every endpoint | polyline w/ styled corners + animated chevrons / shallow rectangular channel |
| Fan | `Fan.tsx` | diameter, direction | `running`, `speed` (rpm→spin), `fault` | `in`, `out` | circle + impeller glyph, spins / short cylinder + spinning blades, status ring |
| Damper | `Damper.tsx` | width, actuated? | `position` (0–100 → blade sweep) | `in`, `out` | blade-in-frame glyph, sweeps / framed rotating vanes |
| Filter | `Filter.tsx` | width, stages | `dp` (pressure drop → dirt tint), `fault` | `in`, `out` | hatched panel / thin slab with hatch texture |
| Coil | `Coil.tsx` | width, medium (chw/hw) | `valve` (0–100), `temp_in`, `temp_out` | `in`, `out` | zigzag glyph tinted by medium / finned slab |
| AHU casing | `AhuCasing.tsx` | w×h, name | `status` (rollup) | none (container) | rounded rect outline + name plate / low extruded enclosure, open top in 3D |

The AHU demo composes exactly these six — the set is complete when the hero shot needs
nothing else.

## Family 2 — Floor plan (`plan.*`)

| Symbol | File | Props | Bindings | Anchors | Flat form / 3D form |
|---|---|---|---|---|---|
| Wall chain | `Wall.tsx` | path points, thickness | — | endpoints + midpoints | crisp double-line / extruded wall (2.7 m default) |
| Room | `Room.tsx` | polygon or w×h, name | `temp` (fill tint via threshold ramp), `occupied` | none | tinted zone fill + name / floor slab tint, works under tilted walls |
| Door | `Door.tsx` | width, swing side | — | snaps **into** a wall segment | arc-swing glyph, cuts the wall / wall gap + leaf |
| Label | `Label.tsx` | text, size, bound value? | optional `value` | none | SDF text, never rotated in flat / billboarded in 3D |

Room `temp` tint is the floor-plan demo's money shot: the office floor breathing with
simulator temperatures.

## Placeholder (not a symbol, but shipped)

An unknown `type` renders the **labeled placeholder box** (dashed outline + type name) —
implemented once in `ShapeNode.tsx`, styled per `look-scope.md`, never a crash. This is
the framework's missing-symbol-pack behavior, proven here.

## Open questions

- Duct medium variants (supply/return/exhaust) as prop-driven tint or separate types?
  (Leaning prop + per-medium accent from `look-scope.md`'s open question.)
- Do rooms want polygon drawing in the playground, or is rect + one L-shape enough to
  judge the look? (Leaning rect-first; polygon only if the demo floor needs it.)
- Damper/fan glyph style: pick by building both against the hero shot in phase 1 —
  this is exactly the kind of decision the playground exists to settle by eye.

## Related

- `look-scope.md` (status palette, materials), `builder-ux-scope.md` (anchors/snap),
  `thecrew-scope.md` (file layout). Framework: symbol packs in
  `docs/scope/frontend/graphics-canvas-scope.md` — this set is drafted as its first pack.
