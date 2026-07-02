// applyPatch purity + the four typed messages; migrate golden (genui-scope Testing plan:
// `applyPatch`/`migrate` purity; IR `v` migration golden files).
import { describe, it, expect } from "vitest";
import { applyPatch, emptySpec } from "./applyPatch";
import { migrate } from "./migrate";
import { IR_VERSION, type IrSpec } from "./types";

describe("applyPatch", () => {
  it("createSurface replaces surface + components + seed data", () => {
    const spec = applyPatch(emptySpec(), {
      type: "createSurface",
      surface: { surfaceId: "cell", root: "r" },
      components: { r: { id: "r", component: "text", props: { value: "hi" } } },
      dataModel: { data: { A: { value: 1 } } },
    });
    expect(spec.surface.root).toBe("r");
    expect(spec.components.r.component).toBe("text");
    expect(spec.dataModel).toEqual({ data: { A: { value: 1 } } });
  });

  it("updateComponents upserts by id", () => {
    const base: IrSpec = {
      v: IR_VERSION,
      surface: { surfaceId: "cell", root: "r" },
      components: { r: { id: "r", component: "stack", children: ["a"] }, a: { id: "a", component: "text" } },
    };
    const next = applyPatch(base, {
      type: "updateComponents",
      components: [{ id: "a", component: "text", props: { value: "new" } }],
    });
    expect(next.components.a.props).toEqual({ value: "new" });
    expect(next.components.r).toBe(base.components.r); // untouched entry shared
    expect(base.components.a.props).toBeUndefined(); // input not mutated
  });

  it("updateDataModel sets a JSON-Pointer path, creating intermediates", () => {
    const base = { ...emptySpec(), dataModel: { data: { A: { value: 1 } } } };
    const next = applyPatch(base, { type: "updateDataModel", pointer: "/data/A/value", value: 99 });
    expect(next.dataModel).toEqual({ data: { A: { value: 99 } } });
    expect(base.dataModel).toEqual({ data: { A: { value: 1 } } }); // pure
    const created = applyPatch(base, { type: "updateDataModel", pointer: "/data/B/rows", value: [1] });
    expect(created.dataModel).toEqual({ data: { A: { value: 1 }, B: { rows: [1] } } });
  });

  it("deleteSurface empties it", () => {
    const base = applyPatch(emptySpec(), {
      type: "createSurface",
      surface: { surfaceId: "cell", root: "r" },
      components: { r: { id: "r", component: "text" } },
    });
    const gone = applyPatch(base, { type: "deleteSurface", surfaceId: "cell" });
    expect(gone.surface.root).toBe("");
    expect(gone.components).toEqual({});
  });
});

describe("migrate", () => {
  it("stamps a version-less legacy blob to v1 and is identity for current v", () => {
    const legacy = { surface: { surfaceId: "cell", root: "r" }, components: {} } as unknown as IrSpec;
    expect(migrate(legacy).v).toBe(1);
    const current: IrSpec = { v: IR_VERSION, surface: { surfaceId: "cell", root: "r" }, components: {} };
    expect(migrate(current)).toEqual(current); // golden: current shape unchanged
  });
  it("returns a future version untouched (validate will flag it)", () => {
    const future = { v: 99, surface: { surfaceId: "cell", root: "r" }, components: {} };
    expect(migrate(future).v).toBe(99);
  });
});
