// The panel wizard (panel-wizard scope, step 4) — gateway test against a REAL spawned gateway + REAL
// seeded rows (rule 9 — no `*.fake.ts`). The wizard is a thin shell over the existing panel model:
//   - `cellToEditorState(defaultCell("timeseries"))` seeds it (the SAME seed ADD uses in the editor);
//   - SourceStep reuses the shipped source picker (entries over the real `series.list`);
//   - ChartTypeStep reuses the shipped VizPicker;
//   - the full-panel preview renders through the SAME `PreviewPane`/`WidgetView` the editor uses.
//
// Headline (step 4): a user picks a real source → the preview shows the seeded value; advances to the
// chart-type step → picks stat → the preview re-renders as a stat panel. No second authoring surface —
// the wizard's `EditorState` is the editor's `EditorState`.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { useRealGateway, signInReal, seedSeries } from "@/test/gateway-session";
import { WithDashboardCache } from "@/features/dashboard/cache/testCacheWrapper";
import { PanelWizard } from "@/features/panel-builder/wizard/PanelWizard";

beforeAll(() => useRealGateway());

let n = 0;
const nextWs = () => `pwiz-${n++}`;

async function seedOne(series: string, payload: number): Promise<void> {
  await seedSeries({ series, seq: 1, payload, key: "kind", value: "temperature" });
}

describe("PanelWizard — source + chart-type steps over real seeded rows (real gateway)", () => {
  it("the wizard mounts at the source step + offers the workspace's real series in the picker", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedOne("cooler.temp", 42);
    const user = userEvent.setup();
    render(
      <WithDashboardCache ws={ws}>
        <PanelWizard ws={ws} dashboardId="d-any" onExit={() => {}} />
      </WithDashboardCache>,
    );
    // The wizard mounts at the source step.
    expect(screen.getByLabelText("wizard source step")).toBeDefined();
    // Click the source combobox to open its options; assert the seeded series is offered.
    await user.click(screen.getByLabelText("wizard source"));
    const opt = await screen.findByRole("option", { name: "cooler.temp" });
    expect(opt).toBeDefined();
    cleanup();
  });

  it("picking a source writes the target through patch + the full-panel preview renders the seeded value", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedOne("cooler.temp", 42);
    const user = userEvent.setup();
    render(
      <WithDashboardCache ws={ws}>
        <PanelWizard ws={ws} dashboardId="d-any" onExit={() => {}} />
      </WithDashboardCache>,
    );
    // Open the source combobox + pick the seeded series entry.
    await user.click(screen.getByLabelText("wizard source"));
    const opt = await screen.findByRole("option", { name: "cooler.temp" });
    await user.click(opt);
    // The wizard's EditorState now carries the picked target; the full-panel preview renders the seeded
    // value through the real WidgetView/usePanelData path (timeseries latest readout).
    await waitFor(() => expect(screen.getByLabelText("timeseries latest")).toBeInTheDocument());
    expect(screen.getByLabelText("timeseries latest").textContent).toContain("42");
    expect(screen.getByLabelText("wizard source picked").textContent).toContain("cooler.temp");
    cleanup();
  });

  it("advancing to the chart-type step + picking stat re-renders the preview as a stat panel", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedOne("cooler.temp", 7);
    const user = userEvent.setup();
    render(
      <WithDashboardCache ws={ws}>
        <PanelWizard ws={ws} dashboardId="d-any" onExit={() => {}} />
      </WithDashboardCache>,
    );
    // Pick a source first (the chart-type step's enablement carries over once a target exists).
    await user.click(screen.getByLabelText("wizard source"));
    const opt = await screen.findByRole("option", { name: "cooler.temp" });
    await user.click(opt);
    await waitFor(() => expect(screen.getByLabelText("timeseries latest")).toBeInTheDocument());

    // Advance to the chart-type step.
    await user.click(screen.getByText("Next"));
    await waitFor(() => expect(screen.getByLabelText("wizard chart-type step")).toBeInTheDocument());

    // Pick the stat view.
    await user.click(screen.getByLabelText("viz stat"));
    // The preview re-renders as a stat panel (aria-label flips from timeseries latest → stat value).
    await waitFor(() => expect(screen.getByLabelText("stat value")).toBeInTheDocument());
    expect(screen.getByLabelText("stat value").textContent).toContain("7");
    expect(screen.getByLabelText("wizard view picked").textContent).toContain("stat");
    cleanup();
  });

  it("the chart-type step mounts the shared Plot editor (PlotBuilder) for plottable views, hides it for stat", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedOne("cooler.temp", 9);
    const user = userEvent.setup();
    render(
      <WithDashboardCache ws={ws}>
        <PanelWizard ws={ws} dashboardId="d-any" onExit={() => {}} />
      </WithDashboardCache>,
    );
    await user.click(screen.getByLabelText("wizard source"));
    await user.click(await screen.findByRole("option", { name: "cooler.temp" }));
    await waitFor(() => expect(screen.getByLabelText("timeseries latest")).toBeInTheDocument());

    await user.click(screen.getByText("Next"));
    await waitFor(() => expect(screen.getByLabelText("wizard chart-type step")).toBeInTheDocument());

    // Default view (timeseries) is plottable — the editor's SAME PlotAxesTab mounts with live fields.
    await waitFor(() => expect(screen.getByLabelText("plot axes tab")).toBeInTheDocument());

    // Switching to a non-plottable view (stat) removes the plot section.
    await user.click(screen.getByLabelText("viz stat"));
    await waitFor(() => expect(screen.queryByLabelText("plot axes tab")).toBeNull());
    cleanup();
  });
});
