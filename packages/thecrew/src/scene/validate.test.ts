// validate.ts unit tests (thecrew-scope.md testing plan): unknown type, missing
// transform, bad bind — never a crash, always a renderable doc + teaching issues.

import { describe, expect, it } from "vitest";
import { validateScene } from "./validate";
import { ahuDemo } from "./demo/ahu-demo";

describe("validateScene", () => {
  it("passes a well-formed doc through untouched (no issues)", () => {
    const { doc, issues } = validateScene(ahuDemo);
    expect(issues).toEqual([]);
    expect(Object.keys(doc.shapes)).toEqual(Object.keys(ahuDemo.shapes));
    expect(doc.camera).toBe("ortho-top");
  });

  it("normalizes garbage input to an empty scene", () => {
    const { doc, issues } = validateScene("not a doc");
    expect(doc).toEqual({ v: 1, camera: "ortho-top", shapes: {} });
    expect(issues.length).toBeGreaterThan(0);
  });

  it("keeps unknown shape types (placeholder render), reports nothing for them", () => {
    const { doc, issues } = validateScene({
      v: 1,
      camera: "ortho-top",
      shapes: { x: { type: "hvac.unobtainium", t: { x: 8, y: 8 }, props: {} } },
    });
    expect(doc.shapes.x.type).toBe("hvac.unobtainium");
    expect(issues).toEqual([]); // unknown type is legal — ShapeNode shows placeholder
  });

  it("defaults a missing transform to origin and reports it", () => {
    const { doc, issues } = validateScene({
      v: 1,
      shapes: { x: { type: "hvac.fan", props: {} } },
    });
    expect(doc.shapes.x.t).toEqual({ x: 0, y: 0 });
    expect(issues.some((i) => i.shapeId === "x" && /transform/.test(i.problem))).toBe(true);
  });

  it("drops malformed binds and keeps good ones", () => {
    const { doc, issues } = validateScene({
      v: 1,
      shapes: {
        x: {
          type: "hvac.fan",
          t: { x: 0, y: 0 },
          props: {},
          bind: { speed: { channel: "ahu1.sf1.speed" }, bad: { chan: "nope" }, worse: 42 },
        },
      },
    });
    expect(doc.shapes.x.bind).toEqual({ speed: { channel: "ahu1.sf1.speed" } });
    expect(issues.filter((i) => i.shapeId === "x")).toHaveLength(2);
  });

  it("reports unknown camera and falls back to ortho-top", () => {
    const { doc, issues } = validateScene({ v: 1, camera: "iso", shapes: {} });
    expect(doc.camera).toBe("ortho-top");
    expect(issues.some((i) => /camera/.test(i.problem))).toBe(true);
  });

  it("replaces a non-object shape with a placeholder", () => {
    const { doc, issues } = validateScene({ v: 1, shapes: { x: 7 } });
    expect(doc.shapes.x.type).toBe("unknown");
    expect(issues.some((i) => i.shapeId === "x")).toBe(true);
  });
});
