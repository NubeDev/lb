import { describe, it, expect } from "vitest";
import { getExtensions } from "../lib/ui/root-ext-stub";
import { lookupWidget, listWidgets } from "./registry";
import "./widgets"; // registers text/value/button

describe("extensions + UI manifest (stub)", () => {
  it("exposes multiple extensions (the outer right-edge tab level)", async () => {
    const exts = await getExtensions();
    expect(exts.length).toBeGreaterThan(1);
    expect(exts.map((e) => e.id)).toContain("ce");
  });

  it("the CE extension's Table + Inspect UIs carry their selection modes and view types", async () => {
    const exts = await getExtensions();
    const ce = exts.find((e) => e.id === "ce")!;
    const byId = Object.fromEntries(ce.uis.map((u) => [u.id, u]));

    expect(byId["components-table"].selection).toBe("sync");
    expect(byId["components-table"].view.type).toBe("collection");
    // Inspect: a follow layout hosting the `inspect` widget.
    expect(byId["components-inspect"].selection).toBe("follow");
    expect(byId["components-inspect"].view.type).toBe("layout");
  });

  it("each extension has at most one collection (Table) UI — one table per extension", async () => {
    const exts = await getExtensions();
    for (const e of exts) {
      const collections = e.uis.filter((u) => u.view.type === "collection");
      expect(collections.length).toBeLessThanOrEqual(1);
    }
  });
});

describe("widget registry", () => {
  it("registers the core widgets and degrades on unknown", () => {
    expect(listWidgets()).toEqual(expect.arrayContaining(["text", "value", "button"]));
    expect(lookupWidget("text")).toBeTypeOf("function");
    expect(lookupWidget("nope")).toBeUndefined();
  });
});
