# thecrew — drag-to-move + canvas lock (session)

## Ask (user, verbatim)

> - move a component and join, same fan one and fan two — I can't move and connect
>   them or align them
> - can we have a button to not let the canvas moving — I need a button to lock it

Two independent pain points in the thecrew scene editor:

1. **No way to reposition a placed shape.** You could place shapes (drag from the
   palette, click, or the chain tool) and nudge with arrow keys, but there was no
   pointer drag on an existing shape. Joining fan-1 to fan-2 was effectively
   impossible: placement snaps onto anchors, but you can't grab a placed fan and
   pull it onto another's anchor.
2. **No canvas lock.** In flat mode `MapControls` pan/zoom is always live, so any
   attempt to drag a part fights the view pan, and there's no way to pin the view.

## Decisions

- **Lock scope = view only** (user chose "lock pan/zoom only"). The lock freezes
  camera pan/zoom; shapes stay selectable and draggable. It's for "I framed my
  view, now let me arrange parts," not a read-only mode.
- **Auto-suppress pan during a drag** (user chose "auto-suppress"). While a shape
  drag is in flight the camera pan/zoom is frozen independently of the lock button,
  so the shape — not the view — follows the cursor. Pan resumes on release.
- **Reuse the placement snap.** Dragging uses the same `snap()` (grid + anchor
  magnetism, anchor wins) the placement path uses, so dropping a dragged fan near
  another fan's anchor snaps them together — this is the "connect" (adjacency = the
  edge, per builder-ux-scope §place). Self-anchors are excluded so a shape can't
  snap to itself.
- **One undo step per drag.** A live drag writes a *transient* position
  (`store.drag`) read only by the shape being dragged; the committed doc mutation
  happens once on pointer-up via `endDrag → moveShapes`. No undo spam per frame.
- **Click vs drag threshold** = 4px of pointer travel. Below it a press stays a
  plain click (select), so selection behavior is unchanged.

## Changes

- `ui/src/state/scene-store.ts` — added ephemeral state `locked` + `drag`, and
  actions `toggleLock`, `setDrag`, `endDrag` (endDrag commits the final position as
  one `moveShapes` step). No new document schema.
- `ui/src/editor/world-anchors.ts` (new) — extracted the local→world anchor
  projection (was private to use-drag-place) so both the placement and move gestures
  share it (one responsibility, no dup).
- `ui/src/editor/use-drag-place.tsx` — now imports the shared `worldAnchors`.
- `ui/src/editor/use-drag-move.tsx` (new) — the move gesture hook: pointer
  down/move/up on a shape, threshold → drag, raycast to z=0 plane, snap, transient
  write, commit on up. Lives in the r3f tree (uses `useThree`).
- `ui/src/canvas/ShapeNode.tsx` — spreads the drag handlers onto the shape group and
  renders the transient drag position while this shape is being dragged.
- `ui/src/canvas/CameraRig.tsx` — `MapControls` pan/zoom (and 3D `OrbitControls`)
  freeze when `locked` or a drag is in flight.
- `ui/src/editor/Toolbar.tsx` — Lock/Unlock icon button + `L` shortcut + keymap row.

## Tests (green)

`ui/src/state/scene-store.test.ts` — 3 new regression tests:
- `toggleLock` flips the view lock without mutating the doc or adding an undo step.
- a drag commits exactly ONE undo step on `endDrag`; transient `setDrag` calls
  mutate nothing and add no steps; the doc is untouched mid-drag; undo restores.
- `endDrag` with no active drag is a no-op (no phantom undo step).

Full UI suite: **72 passed** (was 69). `tsc --noEmit` clean. The single unhandled
error in `scene-render.test.tsx` is a pre-existing offline font-CDN fetch, unrelated.

## Not done / follow-ups

- No capability-deny / workspace-isolation test here: this slice is pure client-side
  editor UI over the local zustand store — it touches no host verb, bus, or store
  key, so those mandatory tests have no surface. Called out per testing-scope §0.
- Multi-select drag: the gesture moves the shape under the cursor only; dragging a
  whole selection together is a follow-up (would extend `endDrag` to the selection).
- Anchor-snap visual affordance during drag (the ghost ring the placement path
  shows) is not rendered for the move gesture yet — the snap still happens, it's
  just not previewed. Nice-to-have.
