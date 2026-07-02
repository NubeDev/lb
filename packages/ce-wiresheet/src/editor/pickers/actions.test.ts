import { describe, it, expect } from "vitest";
import { actionKind, defaultForType, coerceParam } from "./actions";

describe("actionKind", () => {
  it("classifies bool tags", () => {
    expect(actionKind("bool")).toBe("bool");
    expect(actionKind("Boolean")).toBe("bool");
  });
  it("classifies numeric tags across the int/float family", () => {
    for (const t of ["int", "uint32", "i64", "u8", "f32", "float", "double", "number"]) {
      expect(actionKind(t)).toBe("num");
    }
  });
  it("falls back to str", () => {
    expect(actionKind("str")).toBe("str");
    expect(actionKind("whatever")).toBe("str");
  });
});

describe("defaultForType", () => {
  it("returns kind-appropriate zero values", () => {
    expect(defaultForType("bool")).toBe(false);
    expect(defaultForType("i32")).toBe(0);
    expect(defaultForType("str")).toBe("");
  });
});

describe("coerceParam", () => {
  it("parses numbers, defaulting non-finite to 0", () => {
    expect(coerceParam("f64", "3.5")).toBe(3.5);
    expect(coerceParam("i32", "nope")).toBe(0);
  });
  it("parses bool from true/1", () => {
    expect(coerceParam("bool", "true")).toBe(true);
    expect(coerceParam("bool", "1")).toBe(true);
    expect(coerceParam("bool", "0")).toBe(false);
    expect(coerceParam("bool", "false")).toBe(false);
  });
  it("passes strings through", () => {
    expect(coerceParam("str", "hi")).toBe("hi");
  });
});
