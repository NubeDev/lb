# thecrew scope — the look

Status: scope (the ask). The visual bar for the graphics canvas, made concrete enough
to review against. The whole point of `thecrew` is that this doc's bar is met.

The target feeling: **a dark control room, not a diagram tool.** A Niagara PX page says
"engineering document"; this should say "mission control". When a plant graphic is on a
wall display, people who don't know what an AHU is should still stop and look.

## The bar (the screenshot test)

Every phase ends with screenshots into `docs/shots/`. A shot passes when:

1. Nothing looks like a default — no default three.js gray, no unstyled DOM, no browser
   focus rings on canvas chrome.
2. Status is legible from 3 meters: you can tell running/stopped/fault without reading
   a single label.
3. Motion exists but is calm: flow moves, fans spin, values tick — nothing blinks,
   nothing bounces.
4. The flat page could be mistaken for a designed poster; the 3D tilt could be
   mistaken for a game HUD.

## Visual language

- **Theme: deep-space dark.** Near-black blue-gray canvas (`#0a0e14` family), one
  restrained accent (electric cyan) reserved for *live data and selection only*.
  Equipment bodies are desaturated steel/graphite; color means something (status), it
  is never decoration. Light theme is a non-goal here.
- **Status palette (the only saturated colors):** running = cyan/teal glow · stopped =
  dim neutral · fault = amber→red emissive · override/manual = violet. Defined once in
  `theme/tokens.ts`, consumed via `theme/materials.ts`.
- **Depth without noise:** soft ambient occlusion under equipment, a barely-there
  ground grid that fades with zoom, gentle contact shadows. No skybox, no fog gimmicks
  in flat mode.
- **Glow, selectively.** Bloom (via `@react-three/postprocessing`) only on emissive
  status elements and the selection halo — never full-scene. Bloom-on-everything is the
  fastest way to look like a demo, and is called out here as a rejected default.
- **DOM chrome (palette/rail/toolbar):** quiet glass — translucent panels over the
  canvas, hairline borders, Tailwind v4 tokens shared with `styles.css`. The canvas is
  the star; chrome recedes.

## Lighting & materials recipe (flat mode)

One place: `SceneCanvas.tsx` + `theme/materials.ts`.

- Orthographic top-down camera; a soft key light slightly off-axis (so extrusions read
  as depth even "flat") + low ambient; AO from drei.
- Equipment: matte PBR, low metalness, subtle edge highlight on hover.
- Ducts: slightly lighter than background, with an **animated flow texture** (scrolling
  chevrons/dashes, speed bound to the fan value) — the single most important "it's
  alive" cue.
- Text: drei `Text` (SDF) with one typeface, two sizes (label / value), never rotated
  in flat mode. Values get tabular numerals.

## Motion rules

- Camera flat↔3D: one spring transition (~600 ms, no easing gimmicks), everything else
  stays put — the scene must feel like *the same object* seen differently.
- Interaction feedback ≤150 ms; property changes reflect on the shape immediately.
- Fans spin at bound speed; dampers visibly sweep when their value changes; a fault
  pulses emissive at ~0.5 Hz (calm, not alarm-strobe).
- Respect `prefers-reduced-motion`: flow/spin freeze, transitions become fades.

## Anti-goals (things that read as amateur)

- Rainbow status colors, gradients-as-decoration, drop shadows on everything.
- Skeuomorphic 90s SCADA clipart (beveled chrome fans). The symbol language is flat,
  geometric, confident — see `symbols-scope.md`.
- Bloom everywhere; lens flares; parallax wobble.
- Default cursor for everything: place = crosshair, drag = grabbing, connect = cell.

## Open questions

- Does the flat page want a subtle paper/blueprint texture, or pure flat color? (Try
  both in phase 1; decide by screenshot.)
- One accent (cyan) or per-medium accents (air = cyan, water = blue, electric =
  yellow)? Leaning per-medium, capped at three, all defined in tokens.
- SSAO cost on low-end laptops — measure in phase 1; the look must survive with AO off.

## Related

- `thecrew-scope.md` (master), `symbols-scope.md` (the shapes this language styles),
  `builder-ux-scope.md` (chrome behavior). Framework home:
  `docs/scope/frontend/graphics-canvas-scope.md`.
