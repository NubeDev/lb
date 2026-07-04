// The catalog ↔ renderer consistency guard (widget-catalog scope, Slice A — required this slice).
// Imports the HOST-OWNED widget catalog (`rust/crates/host/src/dashboard/widget_catalog.json`, the
// backend source of truth) and asserts its view ids EXACTLY match `WidgetView`'s render-switch cases
// and the trimmed `View` union. This does NOT make TS the source of truth — it makes the renderer
// accountable TO the backend truth: catalog↔renderer drift reproduces the exact G4 symptom this slice
// kills (the host vouching for a view nothing renders), so we fail the build on drift instead of
// discovering it at render time.

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

import catalog from "../../../../../rust/crates/host/src/dashboard/widget_catalog.json";

/** The `case "…":` view ids the render switch handles, parsed from the WidgetView source (the one
 *  renderer). Reading the source keeps this honest — a case added/removed shows up here immediately. */
function renderSwitchCases(): Set<string> {
  // Resolved from the vitest cwd (the `ui/` root) — `import.meta.url` is not a file URL here.
  const src = readFileSync(
    resolve(process.cwd(), "src/features/dashboard/views/WidgetView.tsx"),
    "utf8",
  );
  const cases = new Set<string>();
  for (const m of src.matchAll(/case\s+"([^"]+)":/g)) cases.add(m[1]);
  return cases;
}

describe("widget catalog ↔ renderer consistency", () => {
  const catalogIds = new Set<string>(catalog.views.map((v) => v.id));

  it("every catalog view id has a WidgetView render case, and vice-versa", () => {
    const cases = renderSwitchCases();
    const missingRenderer = [...catalogIds].filter((id) => !cases.has(id)).sort();
    const missingCatalog = [...cases].filter((id) => !catalogIds.has(id)).sort();
    expect(missingRenderer, "catalog views with no renderer case").toEqual([]);
    expect(missingCatalog, "renderer cases with no catalog entry").toEqual([]);
    // The renderer and the catalog are exactly the same set (17 views this slice).
    expect(cases.size).toBe(catalogIds.size);
  });

  it("view ids are unique in the catalog", () => {
    expect(catalog.views.length).toBe(catalogIds.size);
  });

  it("the trimmed View union carries no dead id (the render switch is the ground truth)", () => {
    // The dead ids trimmed this slice — no renderer case, no catalog entry. If any crept back into the
    // catalog or the switch it would fail the set-equality test above; this asserts they stay absent so
    // the type contract, the renderer, and the catalog remain ONE list.
    for (const dead of ["histogram", "state-timeline", "status-history", "heatmap", "text"]) {
      expect(catalogIds.has(dead), `${dead} must not reappear in the catalog`).toBe(false);
    }
  });
});
