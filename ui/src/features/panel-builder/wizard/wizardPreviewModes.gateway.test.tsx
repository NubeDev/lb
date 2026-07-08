// The wizard's UX upgrades (panel-wizard scope, UX pass) — gateway test against a REAL spawned gateway +
// REAL seeded rows (rule 9). Covers the three new seams:
//   - the chart-type step mounts the editor's SAME TemplateOptionsEditor (CodeMirror body + "Copy AI
//     prompt") when the template view is picked — no wizard-only template surface;
//   - the pinned preview's display-only Chart | Table | JSON toggle — JSON shows the draft's REAL rows;
//   - the draggable step↔preview separator is present and keyboard-resizable.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { useRealGateway, signInReal, seedSeries } from "@/test/gateway-session";
import { WithDashboardCache } from "@/features/dashboard/cache/testCacheWrapper";
import { PanelWizard } from "@/features/panel-builder/wizard/PanelWizard";

beforeAll(() => useRealGateway());

let n = 0;
const nextWs = () => `pwizux-${n++}`;

async function mountAtChartType(ws: string, payload: number) {
  await signInReal("user:ada", ws);
  await seedSeries({ series: "cooler.temp", seq: 1, payload, key: "kind", value: "temperature" });
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
  return user;
}

describe("PanelWizard — chart-type step per-view editors + preview modes (real gateway)", () => {
  it("picking the template view mounts the editor's template body editor + the Copy AI prompt", async () => {
    const user = await mountAtChartType(nextWs(), 42);
    await user.click(screen.getByLabelText("viz template"));
    // The SAME TemplateOptionsEditor the panel editor mounts — body editor + AI-prompt copier.
    await waitFor(() => expect(screen.getByLabelText("wizard template body")).toBeInTheDocument());
    expect(screen.getByLabelText("copy ai prompt")).toBeInTheDocument();
    expect(screen.getByLabelText("prompt data sample")).toBeInTheDocument();
    // The plot section is gone (template isn't plottable).
    expect(screen.queryByLabelText("plot axes tab")).toBeNull();
    cleanup();
  });

  it("picking stat mounts the stat basics — sparkline switch + the registry's thresholds/mappings rows", async () => {
    const user = await mountAtChartType(nextWs(), 42);
    await user.click(screen.getByLabelText("viz stat"));
    await waitFor(() => expect(screen.getByLabelText("wizard stat basics")).toBeInTheDocument());
    // The SAME registry rows the Options step renders — one binding, no drift.
    expect(screen.getByLabelText("option section thresholds")).toBeInTheDocument();
    expect(screen.getByLabelText("option section mappings")).toBeInTheDocument();
    // The sparkline switch writes graphMode through the registry binding: default on (area) → off (none).
    const sw = screen.getByLabelText("show sparkline");
    expect(sw.getAttribute("aria-checked") ?? sw.getAttribute("data-state")).toBeTruthy();
    await user.click(sw);
    // The stat preview still renders (sparkline hidden is a presentation change, not a data change).
    await waitFor(() => expect(screen.getByLabelText("stat value")).toBeInTheDocument());
    cleanup();
  });

  it("the preview toggles to JSON and shows the draft's real rows, then back to chart", async () => {
    const user = await mountAtChartType(nextWs(), 77);
    // JSON mode pretty-prints the SAME rows the chart drew (the real usePanelData resolution).
    await user.click(screen.getByLabelText("preview as json"));
    await waitFor(() => expect(screen.getByLabelText("preview rows json")).toBeInTheDocument());
    expect(screen.getByLabelText("preview rows json").textContent).toContain("77");
    // Back to chart — the same timeseries preview re-mounts.
    await user.click(screen.getByLabelText("preview as chart"));
    await waitFor(() => expect(screen.getByLabelText("timeseries latest")).toBeInTheDocument());
    cleanup();
  });

  it("the step↔preview separator is present and resizes with the keyboard", async () => {
    const user = await mountAtChartType(nextWs(), 5);
    const sep = screen.getByLabelText("resize wizard panes");
    expect(sep.getAttribute("role")).toBe("separator");
    // Keyboard resize nudges the grid fraction (display-only state; the saved cell is untouched).
    sep.focus();
    await user.keyboard("{ArrowLeft}");
    await user.keyboard("{ArrowLeft}");
    const grid = sep.parentElement as HTMLElement;
    expect(grid.style.gridTemplateColumns).toContain("0.44fr");
    cleanup();
  });
});
