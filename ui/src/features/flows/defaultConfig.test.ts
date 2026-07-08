// Unit tests for the per-node default-config seed (flows-canvas). The `rhai` template is the load-
// bearing case: a freshly-added rhai node opens with a working payload-router instead of a blank
// source box. The template itself is exercised end-to-end against the real rhai cage in
// `rust/crates/host/tests/flows_debug_test.rs`-style flows; here we prove the seed shape.

import { describe, expect, it } from "vitest";

import { defaultConfig } from "./defaultConfig";

describe("defaultConfig — per-node seed", () => {
  it("seeds a rhai node with a starter source template", () => {
    const cfg = defaultConfig("rhai");
    expect(typeof cfg.source).toBe("string");
    expect((cfg.source as string).length).toBeGreaterThan(0);
    // The template references the payload variable and handles the three shapes.
    expect(cfg.source).toContain("payload");
    expect(cfg.source).toContain("100");
    expect(cfg.source).toMatch(/on/);
    expect(cfg.source).toMatch(/off/);
    expect(cfg.source).toContain("type_of");
  });

  it("returns an empty config for nodes with no starter template", () => {
    expect(defaultConfig("trigger")).toEqual({});
    expect(defaultConfig("debug")).toEqual({});
    expect(defaultConfig("count")).toEqual({});
    expect(defaultConfig("mqtt.publish")).toEqual({});
  });
});
