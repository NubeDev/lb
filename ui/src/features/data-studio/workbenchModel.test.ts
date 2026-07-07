// The workbench-model vocabulary (data-studio-10x scope, phase 1) — the persisted Dockview record +
// its versioned loader. The loader is the migration seam: a tagged dockview record restores; everything
// else (null, a legacy flexlayout blob, a corrupt shape) falls back to the default workbench and raises
// the one-time reset notice. Pure data — no JSX, no React — so a plain Vitest unit (no gateway needed).

import { describe, expect, it } from "vitest";

import {
  DATA_STUDIO_SURFACE,
  loadWorkbench,
  mintTabId,
  viewPaneId,
  workbenchRecord,
} from "./workbenchModel";

describe("workbenchModel — record versioning", () => {
  it("the surface key is the stable 'data-studio' string (the layout record's surface arg)", () => {
    expect(DATA_STUDIO_SURFACE).toBe("data-studio");
  });

  it("viewPaneId mints the deterministic 'view:<kind>' id — one pane per view kind", () => {
    expect(viewPaneId("flows")).toBe("view:flows");
    expect(viewPaneId("rules")).toBe("view:rules");
    // Idempotent: the same kind maps to the same id (a re-open refocuses instead of duplicating).
    expect(viewPaneId("flows")).toBe(viewPaneId("flows"));
  });

  it("mintTabId mints a builder-kind id that is unique across calls (collision-proof)", () => {
    const a = mintTabId("builder");
    const b = mintTabId("builder");
    expect(a).not.toBe(b);
    expect(a.startsWith("builder-")).toBe(true);
    expect(b.startsWith("builder-")).toBe(true);
  });

  it("workbenchRecord wraps a serialized dock with the engine tag", () => {
    const model = { groups: [], panels: [] } as never;
    const rec = workbenchRecord(model);
    expect(rec.engine).toBe("dockview");
    expect(rec.model).toBe(model);
  });

  describe("loadWorkbench — the migration seam", () => {
    it("null saved → default silently (no notice)", () => {
      const out = loadWorkbench(null);
      expect(out.model).toBeNull();
      expect(out.wasReset).toBe(false);
    });

    it("undefined saved → default silently (no notice)", () => {
      // A never-saved record deserializes to `undefined` (no row); same as null.
      const out = loadWorkbench(undefined);
      expect(out.model).toBeNull();
      expect(out.wasReset).toBe(false);
    });

    it("a tagged dockview record restores verbatim (no notice)", () => {
      const model = { groups: [{ id: "g1" }], panels: [] } as never;
      const saved = { engine: "dockview", model };
      const out = loadWorkbench(saved);
      expect(out.model).toBe(model);
      expect(out.wasReset).toBe(false);
    });

    it("a legacy flexlayout blob (no engine tag) → default + reset notice (the v2/v3 → 10x path)", () => {
      // A recognizable flexlayout-era shape: `layout` top-level key, no `engine` tag. The user's
      // drafts inside the old layout are the accepted loss; the library holds anything saved.
      const legacy = { layout: { id: "root", type: "row", children: [] } };
      const out = loadWorkbench(legacy);
      expect(out.model).toBeNull();
      expect(out.wasReset).toBe(true);
    });

    it("a tagged record with a null model is treated as non-migratable → default + reset notice", () => {
      // A real "default workbench" save serializes the empty dock (`toJSON()` returns an object, not
      // null). A `model: null` is therefore a corrupt/hand-edited shape, not a recognized default →
      // the loader falls back and raises the notice (forward-safe: load only what it can verify).
      const out = loadWorkbench({ engine: "dockview", model: null });
      expect(out.model).toBeNull();
      expect(out.wasReset).toBe(true);
    });

    it("a foreign engine tag (e.g. a future 'dockview2') → default + reset notice", () => {
      // An unknown engine is treated as a legacy/non-migratable shape — the studio is forward-safe.
      const out = loadWorkbench({ engine: "dockview2", model: {} });
      expect(out.model).toBeNull();
      expect(out.wasReset).toBe(true);
    });
  });
});
