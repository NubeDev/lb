// The loud ACCEPT step: parse → normalize → validate → size-check, once (genui-scope Testing plan:
// accept-time rejection paths — unparseable emission, over-8 KB spec — fail loudly with stated messages).
import { describe, it, expect } from "vitest";
import { nubeCatalog } from "./catalog/nubeCatalog";
import { acceptLang, acceptIr, GENUI_MAX_BYTES, specByteSize } from "./authoring";
import type { IrSpec } from "./ir/types";
import { IR_VERSION } from "./ir/types";

describe("acceptLang", () => {
  it("accepts a well-formed spec and returns the typed IR (never raw Lang)", () => {
    const res = acceptLang('root = Stat("Flow count", 0)', { catalog: nubeCatalog });
    expect(res.ok).toBe(true);
    expect(res.ir?.surface.root).toBe("root");
    expect(res.ir?.components.root.component).toBe("stat");
  });

  it("rejects an over-8KB spec loudly with the 'simplify' message", () => {
    // Build a big-but-valid spec directly (a huge literal prop).
    const big: IrSpec = {
      v: IR_VERSION,
      surface: { surfaceId: "cell", root: "r" },
      components: { r: { id: "r", component: "text", props: { value: "x".repeat(GENUI_MAX_BYTES + 100) } } },
    };
    expect(specByteSize(big)).toBeGreaterThan(GENUI_MAX_BYTES);
    const res = acceptIr(big, { catalog: nubeCatalog });
    expect(res.ok).toBe(false);
    expect(res.error).toMatch(/too large/i);
    expect(res.error).toMatch(/simplify/i);
  });

  it("rejects a spec whose IR has no root loudly", () => {
    const res = acceptIr(
      { v: IR_VERSION, surface: { surfaceId: "cell", root: "" }, components: {} },
      { catalog: nubeCatalog },
    );
    expect(res.ok).toBe(false);
    expect(res.error).toMatch(/invalid/i);
  });

  it("surfaces normalize warnings even when accept succeeds", () => {
    // An unknown component normalizes to a placeholder (a warning) but still validates.
    const res = acceptIr(
      {
        v: IR_VERSION,
        surface: { surfaceId: "cell", root: "r" },
        components: { r: { id: "r", component: "Bogus" } },
      },
      { catalog: nubeCatalog },
    );
    expect(res.ok).toBe(true); // placeholder is a valid catalog component
    expect(res.findings.some((f) => f.code === "unknown-component")).toBe(true);
  });
});
