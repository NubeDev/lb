// Fit math (parent-scope "fit per dashboard cell" risk). The editor page frames a fixed ±350 world
// units at zoom 1.6; a small read-only cell auto-fits instead. These pin the pure math so the "blank
// cell" regression can't return silently: bounds from shape transforms, a zoom that frames the box into
// the cell pixels (clamped to the page's [0.4, 6]), and the box center to pan to.

import { describe, it, expect } from "vitest";
import { sceneBounds, fitZoom, boundsCenter } from "./fit-bounds";
import type { SceneDoc } from "../scene/scene.types";

function doc(shapes: SceneDoc["shapes"]): SceneDoc {
  return { v: 1, camera: "ortho-top", shapes };
}

describe("sceneBounds", () => {
  it("computes the XY box from shape transforms", () => {
    const d = doc({
      a: { type: "hvac.fan", t: { x: -100, y: 50 }, props: {} },
      b: { type: "hvac.fan", t: { x: 300, y: -20 }, props: {} },
    });
    expect(sceneBounds(d)).toEqual({ minX: -100, minY: -20, maxX: 300, maxY: 50 });
  });

  it("an empty scene yields a small origin box (fit stays well-defined)", () => {
    expect(sceneBounds(doc({}))).toEqual({ minX: -100, minY: -100, maxX: 100, maxY: 100 });
  });
});

describe("fitZoom", () => {
  it("frames a scene offset from the origin so it's visible (not the fixed 1.6 crop)", () => {
    // A scene sitting far from origin in a small cell: the fixed editor zoom would crop it out; the fit
    // zoom is chosen so the padded span fits the cell width/height.
    const b = { minX: 800, minY: 800, maxX: 1200, maxY: 1000 };
    const z = fitZoom(b, 300, 200);
    // span with default pad 120: x = 400+240 = 640, y = 200+240 = 440 → min(300/640, 200/440) ≈ 0.454
    expect(z).toBeGreaterThan(0.4);
    expect(z).toBeLessThan(0.6);
  });

  it("clamps a tiny scene to the max page zoom (never absurdly zoomed in)", () => {
    expect(fitZoom({ minX: -1, minY: -1, maxX: 1, maxY: 1 }, 2000, 2000)).toBe(6);
  });

  it("clamps a huge scene to the min page zoom", () => {
    expect(fitZoom({ minX: -5000, minY: -5000, maxX: 5000, maxY: 5000 }, 200, 200)).toBe(0.4);
  });

  it("a zero-area viewport (pre-layout) falls back to the page default, never Infinity", () => {
    expect(fitZoom({ minX: 0, minY: 0, maxX: 10, maxY: 10 }, 0, 0)).toBe(1.6);
  });
});

describe("boundsCenter", () => {
  it("returns the box center (the ortho pan target — the scene isn't assumed at origin)", () => {
    expect(boundsCenter({ minX: 100, minY: 200, maxX: 300, maxY: 400 })).toEqual({ x: 200, y: 300 });
  });
});
