// The Data page (admin DB browser), driven against a REAL in-process gateway (data-console scope;
// CLAUDE §9 / testing §0 — no fake backend). Each test logs in to a UNIQUE workspace, seeds real rows
// through the real write path, and drives the real `DataView` + hook + api client + HTTP transport.
// Covers: the table picker with counts, selecting a table → paged rows, row-expand → JSON, and the
// Grid/Graph toggle rendering the react-flow view. (The admin-only capability deny is proven
// server-side in the Rust route test; the nav cap-gating is unit-tested in NavGating.test.ts.)

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { DataView } from "./DataView";
import { writeSample } from "@/lib/ingest/ingest.api";
import { useRealGateway, signInReal } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `data-${n++}`;

beforeAll(() => useRealGateway());

describe("DataView (real gateway)", () => {
  it("lists tables with counts and pages a table's rows", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:root", ws);
    await writeSample({ series: "node.cpu_temp", producer: "", ts: 1, seq: 1, payload: 60.0 });
    await writeSample({ series: "node.cpu_temp", producer: "", ts: 2, seq: 2, payload: 61.4 });

    render(<DataView ws={ws} />);

    // The picker shows the real `series` table with its committed row count (2).
    const seriesTable = await screen.findByLabelText("select table series");
    expect(within(seriesTable).getByText("2")).toBeInTheDocument();

    // Selecting it pages the raw rows into the grid (both committed samples).
    await user.click(seriesTable);
    // The grid shows the record ids (composite `series:[…]`).
    const rows = await screen.findAllByLabelText(/^row series:/);
    expect(rows.length).toBe(2);
  });

  it("expands a row to its full JSON", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:root", ws);
    await writeSample({ series: "s", producer: "", ts: 7, seq: 1, payload: 42 });

    render(<DataView ws={ws} />);
    await user.click(await screen.findByLabelText("select table series"));

    // Click the row → its JSON detail appears.
    const row = await screen.findByLabelText(/^row series:/);
    await user.click(row);
    const json = await screen.findByLabelText(/^json series:/);
    expect(json).toHaveTextContent(/"payload": 42/);
    expect(json).toHaveTextContent(/"producer": "user:root"/);
  });

  it("renders the react-flow graph view on toggle", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:root", ws);
    await writeSample({ series: "node.cpu_temp", producer: "", ts: 1, seq: 1, payload: 1 });

    render(<DataView ws={ws} />);
    await user.click(await screen.findByLabelText("select table series"));

    // Flip to the graph view — the lazy-loaded react-flow surface mounts.
    await user.click(screen.getByRole("tab", { name: "Graph" }));
    expect(await screen.findByTestId("data-graph")).toBeInTheDocument();
  });
});
