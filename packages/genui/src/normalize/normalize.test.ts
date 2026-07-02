// normalize: the LLM-sloppiness pass — unknown component → placeholder + WARNING, dangling child →
// dropped + warning, wrong-typed prop → coerced/defaulted + warning (genui-scope Testing plan: normalize
// on sloppy output — the graphics-canvas validate-and-placeholder pattern). It fixes, never throws.
import { describe, it, expect } from "vitest";
import { nubeCatalog } from "../catalog/nubeCatalog";
import { normalize, PLACEHOLDER } from "./normalize";
import type { IrSpec } from "../ir/types";
import { IR_VERSION } from "../ir/types";

function spec(components: IrSpec["components"], root = "r"): IrSpec {
  return { v: IR_VERSION, surface: { surfaceId: "cell", root }, components };
}

describe("normalize", () => {
  it("unknown component → placeholder + warning", () => {
    const { spec: out, findings } = normalize(
      spec({ r: { id: "r", component: "Frobnicate", props: { x: 1 } } }),
      nubeCatalog,
    );
    expect(out.components.r.component).toBe(PLACEHOLDER);
    expect(findings.some((f) => f.code === "unknown-component" && f.level === "warning")).toBe(true);
  });

  it("dangling child id → dropped + warning", () => {
    const { spec: out, findings } = normalize(
      spec({
        r: { id: "r", component: "stack", children: ["a", "ghost"] },
        a: { id: "a", component: "text", props: { value: "hi" } },
      }),
      nubeCatalog,
    );
    expect(out.components.r.children).toEqual(["a"]);
    expect(findings.some((f) => f.code === "dangling-child")).toBe(true);
  });

  it("wrong-typed prop → coerced + warning; missing required with default → defaulted", () => {
    const { spec: out, findings } = normalize(
      spec({ r: { id: "r", component: "gauge", props: { value: 5, min: "0", max: "100" } } }),
      nubeCatalog,
    );
    // gauge min/max are numbers; string "0"/"100" coerce to 0/100.
    expect(out.components.r.props?.min).toBe(0);
    expect(out.components.r.props?.max).toBe(100);
    expect(findings.some((f) => f.code === "coerced-prop")).toBe(true);
  });

  it("leaves a $bind prop intact (its value is resolved later, not coerced now)", () => {
    const { spec: out } = normalize(
      spec({ r: { id: "r", component: "stat", props: { value: { $bind: "/data/A/value" } } } }),
      nubeCatalog,
    );
    expect(out.components.r.props?.value).toEqual({ $bind: "/data/A/value" });
  });

  it("never throws on deeply sloppy input", () => {
    expect(() =>
      normalize(spec({ r: { id: "r", component: "Nope", children: ["x", "y"] } }), nubeCatalog),
    ).not.toThrow();
  });
});
