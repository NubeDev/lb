// Value-mapping usability gate — REAL gateway (editor-parity scope, step 2 testing plan; CLAUDE §9).
// Author a value mapping ENTIRELY through the builder UI over REAL seeded rows (no JSON typed
// anywhere), save it, and assert the rendered STAT shows the MAPPED text — proving the builder writes
// the exact shape the render path applies. The field picker + mapping controls run against a real
// viz.query result. Drives `BuilderPane` directly (data-studio scope v2: authoring moved off the
// dashboard into the inline builder; the option surface is unchanged).

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { BuilderPane } from "@/features/panel-builder/BuilderPane";
import { defaultCell } from "@/lib/panel-kit";
import { defaultOptionsForView } from "@/features/panel-builder/viewOptions";
import { WidgetHost } from "@/features/dashboard/WidgetHost";
import { WithDashboardCache } from "@/features/dashboard/cache/testCacheWrapper";
import { useRealGateway, signInReal, seedSeries } from "@/test/gateway-session";
import type { Cell } from "@/lib/dashboard";

let n = 0;
const nextWs = () => `vmap-${n++}`;

beforeAll(() => useRealGateway());

describe("value-mapping usability (real gateway)", () => {
  it("authors a range mapping through the builder UI and the stat renders the mapped text — no JSON typed", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    // A real single sample so the stat reduces to one scalar value (42) via series.read → viz.query.
    await seedSeries({ series: "map.temp", seq: 1, payload: 42, key: "kind", value: "temperature" });

    // Open the builder on a fresh cell already bound to the seeded series (the pick is exercised by
    // the Data Studio gateway test; here the focus is the mapping controls).
    const cell: Cell = {
      ...defaultCell("stat", "w1", undefined, defaultOptionsForView("stat")),
      sources: [{ refId: "A", tool: "series.read", args: { series: "map.temp" }, datasource: { type: "surreal" } }],
      source: { tool: "series.read", args: { series: "map.temp" } },
    };
    let saved: Cell | null = null;
    render(
      <WithDashboardCache ws={ws}>
        <BuilderPane ws={ws} cell={cell} onSave={(c) => (saved = c)} />
      </WithDashboardCache>,
    );

    // Field tab → add a RANGE value mapping covering the value, display text "MAPPED". Entirely via UI.
    await user.click(await screen.findByText("Field"));
    await user.click(await screen.findByLabelText("add range mapping"));
    await user.type(screen.getByLabelText("mapping 0 from"), "0");
    await user.type(screen.getByLabelText("mapping 0 to"), "100");
    await user.type(screen.getByLabelText("mapping 0 text"), "MAPPED");
    await user.click(screen.getByLabelText("mapping 0 color green"));

    await user.click(screen.getByLabelText("save panel"));

    // Render the SAVED cell through the shipped render path: it shows the MAPPED text (not the raw 42),
    // proving the builder wrote the exact shape `fieldconfig/mappings.ts` applies, end to end.
    expect(saved).not.toBeNull();
    render(
      <WithDashboardCache ws={ws}>
        <WidgetHost cell={saved!} workspace={ws} installed={[]} />
      </WithDashboardCache>,
    );
    expect(await screen.findByText("MAPPED", {}, { timeout: 4000 })).toBeInTheDocument();
  });
});
