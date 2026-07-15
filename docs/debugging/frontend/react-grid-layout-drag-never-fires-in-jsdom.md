# react-grid-layout drag never fires onDragStop in jsdom ("onDragEnd called before onDragStart")

- **Area:** frontend / `packages/dashboard` tests
- **Date:** 2026-07-15
- **Symptom:** a vitest jsdom test that drives a real drag (mousedown on the
  `.lbdg-drag-handle` → mousemove on `document` → mouseup) never gets `onLayout`; the console
  shows `Error: onDragEnd called before onDragStart` from `react-grid-layout`'s `GridItem`.

## Cause

jsdom performs no layout, so `HTMLElement.offsetParent` is always `null`. `GridItem.onDragStart`
early-returns when `offsetParent` is falsy, so the drag never *starts* — but `DraggableCore`
still fires its stop handler on mouseup, which `GridItem` rejects with the error above. The
grid's `onDragStop` (→ our `mergeLayout` → `onLayout`) is never reached.

## Fix

Shim the environment gap in the test file (NOT a mock of grid behavior — the real
react-grid-layout/react-draggable code path runs end to end):

```ts
beforeAll(() => {
  Object.defineProperty(HTMLElement.prototype, "offsetParent", {
    get() { return (this as HTMLElement).parentElement; },
  });
});
```

Second trap in the same test: **vertical compaction**. Dragging the top item down compacts it
right back to `y:0`, so the merged payload shows no movement and assertions on "it moved" fail.
Drag an item UP past another (a real reorder) and assert **relative Δy**, not absolute rows.

## Regression test

`packages/dashboard/src/Grid.test.tsx` — "onLayout receives the FULL cells payload with merged
geometry (the persistence seam)" (the real-drag test itself is the regression test).
