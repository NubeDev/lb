// Value-mapping usability gate — REAL gateway (editor-parity scope, step 2 testing plan; CLAUDE §9).
// Author a value mapping ENTIRELY through the editor UI over REAL seeded rows (no JSON typed anywhere),
// save it, and assert the rendered STAT shows the MAPPED text — proving the editor writes the exact
// shape the render path applies. The field picker + mapping controls run against a real viz.query result.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { DashboardView } from "../../DashboardView";
import { useRealGateway, signInReal, seedSeries } from "@/test/gateway-session";
import { RoutingContextProvider } from "@/features/routing/RoutingContextProvider";
import { getSession } from "@/lib/session";

let n = 0;
const nextWs = () => `vmap-${n++}`;

beforeAll(() => useRealGateway());

function renderDashboard(ws: string) {
  const s = getSession();
  return render(
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
      <DashboardView ws={ws} />
    </RoutingContextProvider>,
  );
}

describe("value-mapping usability (real gateway)", () => {
  it("authors a range mapping through the UI and the stat renders the mapped text — no JSON typed", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    // A real single sample so the stat reduces to one scalar value (42) via series.read → viz.query.
    await seedSeries({ series: "map.temp", seq: 1, payload: 42, key: "kind", value: "temperature" });

    renderDashboard(ws);
    await user.type(await screen.findByLabelText("new dashboard title"), "Maps");
    await user.click(screen.getByLabelText("create dashboard"));

    // Add a panel, bind the real series, switch the viz to Stat (so a mapping renders as text).
    await user.click(await screen.findByLabelText("add panel"));
    await screen.findByRole("option", { name: "map.temp" });
    await user.selectOptions(await screen.findByLabelText("panel source"), "series:map.temp");
    await user.click(screen.getByLabelText("viz stat"));

    // Field tab → add a RANGE value mapping covering the value, display text "MAPPED". Entirely via UI.
    await user.click(screen.getByLabelText("Field"));
    await user.click(await screen.findByLabelText("add range mapping"));
    await user.type(screen.getByLabelText("mapping 0 from"), "0");
    await user.type(screen.getByLabelText("mapping 0 to"), "100");
    await user.type(screen.getByLabelText("mapping 0 text"), "MAPPED");
    await user.click(screen.getByLabelText("mapping 0 color green"));

    await user.click(screen.getByLabelText("save panel"));

    // The saved stat renders the MAPPED text (not the raw 42) — the editor wrote the exact shape the
    // render path (`fieldconfig/mappings.ts`) applies, proven end to end over the real gateway.
    expect(await screen.findByText("MAPPED", {}, { timeout: 4000 })).toBeInTheDocument();
  });
});
