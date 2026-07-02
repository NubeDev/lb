import { describe, expect, it } from "vitest";
import { sanitizeName, uniqueName } from "./naming";

describe("sanitizeName", () => {
  it("takes the local segment after ::", () => {
    expect(sanitizeName("NubeIO-math::add")).toBe("add");
  });
  it("strips characters the engine name validator rejects", () => {
    expect(sanitizeName("core-extRoot::My Node!")).toBe("MyNode");
    expect(sanitizeName("v::a.b-c")).toBe("abc");
  });
  it("falls back to 'node' when nothing survives", () => {
    expect(sanitizeName("x::!!!")).toBe("node");
  });
  it("handles a type with no :: segment", () => {
    expect(sanitizeName("plain")).toBe("plain");
  });
});

describe("uniqueName", () => {
  it("returns the base when free", () => {
    expect(uniqueName("add", new Set())).toBe("add");
  });
  it("appends the first free numeric suffix", () => {
    expect(uniqueName("add", new Set(["add"]))).toBe("add2");
    expect(uniqueName("add", new Set(["add", "add2", "add3"]))).toBe("add4");
  });
  it("accepts any iterable of taken names", () => {
    expect(uniqueName("group", ["group", "group2"])).toBe("group3");
  });
});
