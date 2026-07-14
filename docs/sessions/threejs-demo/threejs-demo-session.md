# Session: threejs-demo — interactive 3D gas station

**Scope:** `threejs-demo/SCOPE.md`
**Date:** 2026-07-14

## What was built

Completed the standalone three.js demo (`threejs-demo/`): a 3D gas station you can
orbit, with clickable hotspot badges over each asset (solar, HVAC, fuel canopy, EV
chargers, refrigeration, car wash, shop, switchboard) showing fake live values, and a
detail drawer with a sparkline. `index.html`, `styles.css`, `models/README.md`, and
`src/data.js` already existed; this session added the remaining modules:

- `src/stage.js` — renderer / scene / lights / camera / OrbitControls, per-frame hook.
- `src/station.js` — loads `models/station.glb` if present, else a procedural station
  (canopy + solar array, pump islands, shop, cool-room, EV bay, car wash, switchboard,
  price sign). Logs GLB part names to line hotspots up.
- `src/layout.js` — asset id → 3D anchor for each hotspot.
- `src/hotspots.js` — DOM badges reprojected onto the canvas every frame; fade when
  behind the camera; click → drawer.
- `src/drawer.js` — detail panel + canvas sparkline (status-coloured, min/max meta).
- `src/main.js` — wires stage/station/hotspots/drawer/data + clock + hint fade.

No backend, no build step, no dependencies — three.js from the unpkg CDN via importmap,
served with `python3 -m http.server 5180`.

## How it was tested (real, no mocks — rule 9)

`node --check` on all seven modules, then drove the served page in **real headless
Chrome** (Playwright): page loads with zero console errors (besides the expected 404
probe for the optional `models/station.glb` + favicon), 8 hotspot badges render with
live values, clicking the Refrigeration badge opens the drawer with the right title,
`critical` pill, and sparkline. Screenshots verified visually.

Capability-deny / workspace-isolation tests are **N/A**: this is a static, backend-less
demo outside the Rust workspace and UI shell — no capabilities, no workspaces, no node.

## Fixes made along the way

- Brightened the scene (`toneMappingExposure` 1.35, stronger hemisphere light) — first
  screenshot was too dark to read the buildings.
- Moved the car wash from behind the shop-side of the canopy to open ground on the left
  (`station.js` + `layout.js` anchor) — its badge was floating over the canopy roof and
  colliding with the Solar badge.

## Open items

- No real `station.glb` in the repo (intentional — see `models/README.md`).
- The demo is untracked (`threejs-demo/` is new); commit when wanted.
