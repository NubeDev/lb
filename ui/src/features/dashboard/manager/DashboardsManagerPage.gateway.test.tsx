// The Dashboards manager page, driven against a REAL in-process gateway (dashboard scope; CLAUDE §9).
// Covers the full-CRUD library surface: it lists seeded dashboards, creates one, imports a pasted
// bundle (which appears in the table), and duplicates a row — all through the shipped `dashboard.*`
// verbs + the real HTTP transport, admin caps from the real signed session. Export opens the shared
// JSON popout (view / copy / download); its store round-trip is covered by `io/dashboardIo.gateway.test.tsx`.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { DashboardsManagerPage } from "./DashboardsManagerPage";
import {
  saveDashboard,
  makeBundle,
  serializeBundle,
  dashboardToPortable,
  getDashboard,
} from "@/lib/dashboard";
import type { Cell } from "@/lib/dashboard";
import { useRealGateway, signInReal } from "@/test/gateway-session";
import { RoutingContextProvider } from "@/features/routing/RoutingContextProvider";
import { ThemeProvider } from "@/lib/theme";
import { getSession } from "@/lib/session";

let n = 0;
const nextWs = () => `dash-mgr-${n++}`;

beforeAll(() => useRealGateway());

const cell: Cell = {
  i: "w1",
  x: 0,
  y: 0,
  w: 8,
  h: 4,
  widget_type: "chart",
  binding: { series: "temp" },
};

function renderManager(ws: string) {
  const s = getSession();
  return render(
    <ThemeProvider>
      <RoutingContextProvider
        value={{
          workspace: ws,
          principal: s?.principal ?? "",
          caps: s?.caps,
          allowed: ["dashboards"],
          extPages: [],
          extPagesLoading: false,
          onSignOut: () => {},
          switchWorkspace: () => {},
        }}
      >
        <DashboardsManagerPage ws={ws} onOpen={() => {}} />
      </RoutingContextProvider>
    </ThemeProvider>,
  );
}

describe("DashboardsManagerPage (real gateway)", () => {
  it("lists seeded dashboards and imports a pasted bundle", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await saveDashboard("ops", "Ops board", [cell]);

    renderManager(ws);
    expect(await screen.findByText("Ops board")).toBeInTheDocument();

    // Import a bundle carrying a second dashboard by pasting JSON into the dialog.
    const full = await getDashboard("ops");
    const bundle = serializeBundle(
      makeBundle(
        [
          {
            ...dashboardToPortable(full),
            id: "imported",
            title: "Imported board",
          },
        ],
        [],
      ),
    );
    await user.click(screen.getByRole("button", { name: /import/i }));
    // Paste (not type) — the JSON has `{`/`[` which userEvent.type reads as special key syntax.
    const textarea = await screen.findByLabelText("bundle json");
    await user.click(textarea);
    await user.paste(bundle);
    // The preview headline shows what the bundle carries; confirm import.
    expect(await screen.findByText(/ready to import/i)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Import" }));

    // The done summary reports the created record; closing shows it in the table.
    await user.click(await screen.findByRole("button", { name: /done/i }));
    expect(await screen.findByText("Imported board")).toBeInTheDocument();
  });

  it("creates a new dashboard from the toolbar", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);

    renderManager(ws);
    await user.click(
      await screen.findByRole("button", { name: /new dashboard/i }),
    );
    await user.type(screen.getByLabelText("new dashboard title"), "Fresh");
    await user.click(screen.getByRole("button", { name: "Create" }));

    expect(await screen.findByText("Fresh")).toBeInTheDocument();
    // The real record exists.
    const created = await getDashboard("fresh");
    expect(created.title).toBe("Fresh");
  });

  it("opens the export popout showing the bundle JSON (view / copy / download)", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await saveDashboard("ops", "Ops board", [cell]);

    renderManager(ws);
    // The per-row export icon opens the shared JSON popout with the real bundle bytes.
    await user.click(await screen.findByLabelText("export Ops board"));

    const payload = await screen.findByLabelText("json payload");
    expect(payload.textContent).toContain(
      '"kind": "lazybones.dashboard-bundle"',
    );
    expect(payload.textContent).toContain("Ops board");
    // The popout offers a copy affordance + a download.
    expect(
      screen.getByRole("button", { name: /copy json/i }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /download/i }),
    ).toBeInTheDocument();
  });
});
