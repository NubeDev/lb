# scene widget cell renders blank — camera framing doesn't fit the dashboard cell

- **Area:** frontend (thecrew graphics-canvas `[[widget]]`)
- **Status:** resolved
- **Date:** 2026-07-02
- **Symptom:** the `ext:thecrew/scene` dashboard cell mounts in-process, the `<canvas>` is attached at a
  real size (probed: 493×516), the scene doc loads over the bridge (`scene-widget` renders, not the empty
  state) — but the cell is **visually blank**. The same scene renders perfectly on the full-page `[ui]`
  view. So: tier/read-only contract holds, data flows, WebGL works (the page proves it) — only the CELL is
  empty.

## Root cause (two layers)

1. **Fixed framing, not fit-to-cell.** The editor page frames a fixed ±350 world units at a fixed ortho
   `zoom={1.6}` (`CameraRig`'s `<OrthographicCamera>`), which is right for a large page but shows only a
   tiny center crop in a small grid cell — and the AHU-1 scene's shapes span x∈[-272,232], so at zoom 1.6
   in a few-hundred-px cell almost everything falls outside the frustum → blank. The fix must **fit** the
   scene's bounding box into the actual cell pixels (and pan to the scene center — the scene isn't at the
   origin), per the parent scope's "WebGL contexts / fit per dashboard cell" risk.

2. **A one-shot `useEffect` fit gets clobbered by drei.** The first fix computed the fit zoom/center in a
   `useEffect` on `[doc, camera, size]`. It STILL rendered blank. Cause: drei's
   `<OrthographicCamera makeDefault position={…} zoom={1.6}>` **re-applies its declarative props on every
   re-render**, overwriting whatever the effect set — the effect wins once, then the next render resets the
   camera to the page default. A one-shot effect can't own a pose that a declarative drei camera keeps
   re-asserting.

## Fix

- New pure `canvas/fit-bounds.ts`: `sceneBounds(doc)` (XY box from shape transforms; empty → small origin
  box), `fitZoom(bounds, w, h, pad)` (frame the padded span into the cell px, clamped to the page's
  `[0.4, 6]`; zero-area viewport → the page default 1.6, never Infinity), `boundsCenter(bounds)`.
- New `canvas/FitCamera.tsx`, mounted INSIDE `<Canvas>`: drives the ortho camera's `zoom`/`position`
  **every frame** via `useFrame` (a handful of float ops, only touching the camera when the frame differs),
  so drei's re-applied props can't win. No-op unless the default camera is an `OrthographicCamera`.
- `SceneCanvas` takes a `fit` prop; the read-only widget passes `<SceneCanvas fit />`, `CameraRig` omits
  MapControls in fit mode (a cell has no pan/zoom). The editor page leaves `fit` off — **its framing +
  morph are untouched** (additive).

## Verified

Live (real node in-mem :8080, built shell :4173, thecrew published + seeded): the AHU-1 airflow train
renders framed + centered in both the seeded `scene-dash` cell AND a cell **added through the restored
builder palette** on `scene-build` — `docs/shots/scene-widget-dashboard.png`. Unit: `fit-bounds.test.ts`
(8) pins the math (offset scene visible, tiny/huge clamps, zero-viewport fallback, center). The e2e now
waits for `scene-widget` + a settle before the screenshot so it captures the framed scene, not a mid-mount
blank frame.

## Lesson

A dashboard cell needs **fit-to-cell** framing, not the page's fixed zoom — compute zoom from the scene
bounds and the actual cell pixels. And when a drei camera is `makeDefault` with declarative
`zoom`/`position`, own the pose in `useFrame`, not a one-shot effect: drei re-asserts its props every
render and silently clobbers an effect-set pose.
