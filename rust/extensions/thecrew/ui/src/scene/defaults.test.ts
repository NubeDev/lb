// defaults.ts unit tests: every known type drops renderable, grid-friendly defaults;
// unknown types still return a placeholder-renderable shape (never throw).

import { describe, expect, it } from "vitest";
import { defaultShape, KNOWN_TYPES } from "./defaults";
import { validateScene } from "./validate";

describe("defaultShape", () => {
  it("covers both symbol families", () => {
    expect(KNOWN_TYPES.filter((t) => t.startsWith("hvac."))).toHaveLength(6);
    expect(KNOWN_TYPES.filter((t) => t.startsWith("plan."))).toHaveLength(4);
  });

  it("every known type validates cleanly", () => {
    for (const type of KNOWN_TYPES) {
      const shape = defaultShape(type);
      expect(shape.type).toBe(type);
      const { issues } = validateScene({ v: 1, camera: "ortho-top", shapes: { s: shape } });
      expect(issues).toEqual([]);
    }
  });

  it("returns fresh objects (no shared mutable state between drops)", () => {
    const a = defaultShape("hvac.duct");
    const b = defaultShape("hvac.duct");
    expect(a).not.toBe(b);
    (a.props.points as number[][])[0][0] = 999;
    expect((b.props.points as number[][])[0][0]).toBe(0);
  });

  it("unknown type yields a placeholder shape, never throws", () => {
    const s = defaultShape("hvac.unobtainium");
    expect(s.type).toBe("hvac.unobtainium");
    expect(s.t).toEqual({ x: 0, y: 0 });
  });
});
