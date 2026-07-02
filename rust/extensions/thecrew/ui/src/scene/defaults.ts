// Per-symbol-type defaults: what the palette drops onto the canvas
// (builder-ux-scope.md §place). One entry per type in symbols-scope.md's tables.
// These prop names ARE the contract between demos, shape components, and the rail —
// change them here and everywhere at once.

import type { SceneShape } from "./scene.types";

/** Deep-cloneable default per type. Unknown type still returns a placeholder-renderable
 * shape (never throws — same posture as validate.ts). */
const DEFAULTS: Record<string, () => SceneShape> = {
  "hvac.duct": () => ({
    type: "hvac.duct",
    t: { x: 0, y: 0 },
    props: { points: [[0, 0], [96, 0]], width: 40, medium: "air", label: "" },
  }),
  "hvac.fan": () => ({
    type: "hvac.fan",
    t: { x: 0, y: 0 },
    props: { diameter: 64, direction: "right", label: "Fan" },
  }),
  "hvac.damper": () => ({
    type: "hvac.damper",
    t: { x: 0, y: 0 },
    props: { width: 48, actuated: true, label: "Damper" },
  }),
  "hvac.filter": () => ({
    type: "hvac.filter",
    t: { x: 0, y: 0 },
    props: { width: 48, stages: 1, label: "Filter" },
  }),
  "hvac.coil": () => ({
    type: "hvac.coil",
    t: { x: 0, y: 0 },
    props: { width: 48, medium: "chw", label: "Coil" },
  }),
  "hvac.casing": () => ({
    type: "hvac.casing",
    t: { x: 0, y: 0 },
    props: { w: 320, h: 128, name: "AHU", label: "" },
  }),
  "plan.wall": () => ({
    type: "plan.wall",
    t: { x: 0, y: 0 },
    props: { points: [[0, 0], [96, 0]], thickness: 8, label: "" },
  }),
  "plan.room": () => ({
    type: "plan.room",
    t: { x: 0, y: 0 },
    props: { w: 96, h: 80, name: "Room", label: "" },
  }),
  "plan.door": () => ({
    type: "plan.door",
    t: { x: 0, y: 0 },
    props: { width: 32, swing: "left", label: "" },
  }),
  "plan.label": () => ({
    type: "plan.label",
    t: { x: 0, y: 0 },
    props: { text: "Label", size: 12, label: "" },
  }),
};

export function defaultShape(type: string): SceneShape {
  const make = DEFAULTS[type];
  if (make) return make();
  // Unknown type: place a placeholder box, never crash (symbols-scope §placeholder).
  return { type, t: { x: 0, y: 0 }, props: {} };
}

/** The palette's catalog: every placeable type, in display order. */
export const KNOWN_TYPES = Object.keys(DEFAULTS);
