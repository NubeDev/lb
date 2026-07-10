// Reports builder unit tests (jsdom, no gateway — the real-gateway round-trip is Track C's job). These
// mount the roster + editor with the `report/brand/panel` clients mocked (no network in jsdom) and
// assert: the roster empty state, the add-block flow updates state, and — the known provider gotcha —
// a panel block renders through PanelEmbed, which mounts its required DashboardCacheProvider (WidgetHost
// / useDatasourceList break without it). We do NOT fake node behavior (§9): the mocks stand in only for
// the transport in a jsdom unit test; the CRUD/render path is exercised for real under test:gateway.

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

// Client transports mocked (jsdom has no node). These are the seams, not node re-implementations.
vi.mock("@/lib/report", async (orig) => {
  const actual = await orig<typeof import("@/lib/report")>();
  return {
    ...actual,
    listReports: vi.fn().mockResolvedValue([]),
    getReport: vi.fn().mockResolvedValue({
      id: "q3",
      title: "Q3",
      owner: "user:ada",
      visibility: "private",
      blocks: [],
      brandId: "default",
      toolbar: {},
      updated_ts: 1,
    }),
    saveReport: vi.fn().mockResolvedValue({ id: "q3", title: "Q3", blocks: [], brandId: "default" }),
    deleteReport: vi.fn().mockResolvedValue(undefined),
  };
});
vi.mock("@/lib/brand", async (orig) => {
  const actual = await orig<typeof import("@/lib/brand")>();
  return {
    ...actual,
    listBrands: vi.fn().mockResolvedValue([
      { id: "default", name: "Default", logoAssetId: "", colors: { primary: "#123456", accent: "#abcdef", text: "#000", background: "#fff" }, fonts: { heading: "Libertinus Serif", body: "Libertinus Serif" }, headerText: "", footerText: "" },
    ]),
    getBrand: vi.fn().mockResolvedValue({ id: "default", name: "Default", logoAssetId: "", colors: { primary: "#123456", accent: "#abcdef", text: "#000", background: "#fff" }, fonts: { heading: "Libertinus Serif", body: "Libertinus Serif" }, headerText: "", footerText: "" }),
  };
});
vi.mock("@/lib/panel", async (orig) => {
  const actual = await orig<typeof import("@/lib/panel")>();
  return { ...actual, listPanels: vi.fn().mockResolvedValue([]) };
});

import { ReportsPage } from "./ReportsPage";
import { ReportEditor } from "./ReportEditor";
import { PanelEmbed } from "@/features/panel/PanelEmbed";
import type { Cell } from "@/lib/dashboard";

afterEach(cleanup);

describe("ReportsPage", () => {
  it("renders the empty roster state when no reports exist", async () => {
    render(<ReportsPage ws="acme" onOpen={() => {}} />);
    expect(await screen.findByText("No reports yet.")).toBeInTheDocument();
  });
});

describe("ReportEditor", () => {
  it("adds a markdown block — the add-block flow updates state (a block row appears)", async () => {
    const user = userEvent.setup();
    render(<ReportEditor ws="acme" id="q3" onClose={() => {}} />);
    // Wait for the loaded (empty) report.
    await screen.findByText("No blocks yet — add text, a panel, or an image.");
    await user.click(screen.getByRole("button", { name: /Text/ }));
    // The block list now carries one markdown block.
    expect(await screen.findByTestId("block-0")).toBeInTheDocument();
    expect(screen.getByTestId("block-list")).toBeInTheDocument();
  });
});

describe("PanelEmbed (the provider gotcha)", () => {
  it("mounts its DashboardCacheProvider so WidgetHost renders (a ready cell renders without a provider crash)", async () => {
    // A minimal v1 cell — renders through WidgetHost WITHOUT throwing only because PanelEmbed supplies
    // the DashboardCacheProvider (useDatasourceList / the read cache need it — the known gotcha).
    const cell: Cell = { i: "c1", x: 0, y: 0, w: 12, h: 8, widget_type: "chart", binding: { series: "" } };
    render(<PanelEmbed ws="acme" cell={cell} />);
    await waitFor(() => expect(screen.getByTestId("panel-embed")).toBeInTheDocument());
  });
});
