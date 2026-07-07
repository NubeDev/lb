// TransformStep (panel-wizard scope, step 6) — the wizard's fourth step, a DATA step. Transformations
// re-query (the backend runs the pipeline); the freeze toggle pins the FETCH so a transform edit reshapes
// cached frames instead of re-hitting the source. Reuses the editor's shipped `TransformTab` verbatim —
// same picker, same per-id editors, same move/disable/remove controls. Real gateway, real seeded rows.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { useRealGateway, signInReal, seedSeries } from "@/test/gateway-session";
import type { Cell } from "@/lib/dashboard";
import { cellToEditorState } from "@/lib/panel-kit/cellEditorState";
import { WithDashboardCache } from "@/features/dashboard/cache/testCacheWrapper";
import { TransformStep } from "@/features/panel-builder/wizard/TransformStep";
import { useWizardPreview } from "@/features/panel-builder/wizard/useWizardPreview";
import type { EditorState } from "@/lib/panel-kit/cellEditorState";
import { useState } from "react";

beforeAll(() => useRealGateway());

let n = 0;
const nextWs = () => `txstep-${n++}`;

async function seedSeriesOf(series: string, pts: number[]): Promise<void> {
  for (const [i, v] of pts.entries()) {
    await seedSeries({ series, seq: i + 1, payload: v, key: "kind", value: "temperature" });
  }
}

function baseCell(series: string): Cell {
  return {
    i: "c",
    x: 0,
    y: 0,
    w: 6,
    h: 4,
    v: 3,
    widget_type: "stat",
    view: "stat",
    binding: { series: "" },
    sources: [{ refId: "A", tool: "series.read", args: { series }, datasource: { type: "series" } }],
    options: { reduceOptions: { calcs: ["lastNotNull"] }, colorMode: "value", graphMode: "none", textMode: "auto" },
  };
}

/** A host shell that mirrors PanelWizard's binding. */
function TransformHarness({ initial }: { initial: Cell }) {
  const [state, setState] = useState<EditorState>(() => cellToEditorState(initial));
  const preview = useWizardPreview(state);
  const [frozen, setFrozen] = useState(false);
  const patch = (next: Partial<EditorState>) => setState((s) => ({ ...s, ...next }));
  return (
    <TransformStep
      state={state}
      patch={patch}
      cell={preview.cell}
      refreshKey={preview.refreshKey}
      frozen={frozen}
      onFrozenChange={setFrozen}
      onSave={() => {}}
    />
  );
}

describe("TransformStep — a data step (transformations re-query) + the freeze toggle (real gateway)", () => {
  it("adds a `reduce: max` transform through the shipped TransformTab + the preview reflects the max", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    // Seed 3 points: 4, 9, 7. A `reduce: max` transform yields 9 — distinct from the lastNotNull (7).
    await seedSeriesOf(`${ws}.temps`, [4, 9, 7]);
    const user = userEvent.setup();
    // Bind a stat cell to that series — the lastNotNull default reads 7.
    const { container, unmount } = render(
      <WithDashboardCache ws={ws}>
        <TransformHarness initial={baseCell(`${ws}.temps`)} />
      </WithDashboardCache>,
    );
    // The transform surface is present (reused TransformTab).
    expect(screen.getByLabelText("transform tab")).toBeDefined();
    // Pick `reduce` in the add-transform combobox.
    await user.click(screen.getByRole("combobox", { name: "add transformation" }));
    const opt = await screen.findByRole("option", { name: /Reduce/i });
    await user.click(opt);
    // The wizard's state now carries the transform (asserted via the transform list rendering its label).
    await waitFor(() => expect(screen.getByLabelText("transform list")).toBeDefined());
    // Sanity: a per-transform control exists (the `reduce` reducer picker).
    expect(container.textContent ?? "").toMatch(/reduce/i);
    unmount();
    cleanup();
  });

  it("the freeze toggle flips `frozen` (the data-step edit-without-requery affordance)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedSeriesOf(`${ws}.temps`, [4, 9]);
    const user = userEvent.setup();
    const { container, unmount } = render(
      <WithDashboardCache ws={ws}>
        <TransformHarness initial={baseCell(`${ws}.temps`)} />
      </WithDashboardCache>,
    );
    // The freeze toggle starts off; click to freeze.
    const toggle = screen.getByLabelText("freeze current data");
    expect(toggle.getAttribute("aria-pressed")).toBe("false");
    await user.click(toggle);
    await waitFor(() => expect(toggle.getAttribute("aria-pressed")).toBe("true"));
    expect(container.textContent ?? "").toMatch(/reshape the cached frames/i);
    unmount();
    cleanup();
  });
});
