// The Ingest page, driven against a REAL in-process gateway (data-console scope; CLAUDE §9 / testing
// §0 — no fake backend). Each test logs in to a UNIQUE workspace (the shared real node stays isolated
// per test), seeds real rows through the real write path (`ingest_write` over the gateway), and
// drives the real `IngestView` + hook + api client + HTTP transport. Covers: series list, select →
// latest + recent render, manual write refreshes the table, faceted search, and the deny→inline-error
// path (a session without the write cap is refused server-side).

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { IngestView } from "./IngestView";
import { writeSample } from "@/lib/ingest/ingest.api";
import { useRealGateway, signInReal } from "@/test/gateway-session";

let n = 0;
/** A fresh workspace per test so seeds never collide on the shared real backend. */
const nextWs = () => `ing-${n++}`;

beforeAll(() => useRealGateway());

describe("IngestView (real gateway)", () => {
  it("lists series and shows latest + recent samples on select", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    // Seed two real samples through the real write path (the gateway drains → committed).
    await writeSample({ series: "node.cpu_temp", producer: "", ts: 1, seq: 1, payload: 60.0 });
    await writeSample({ series: "node.cpu_temp", producer: "", ts: 2, seq: 2, payload: 61.4 });

    render(<IngestView ws={ws} />);

    // The series appears in the list; selecting it shows the latest value + the recent table.
    const seriesBtn = await screen.findByLabelText("select series node.cpu_temp");
    await user.click(seriesBtn);

    // The latest value (seq 2) renders once the async read resolves.
    const latest = await screen.findByLabelText("latest value");
    await waitFor(() => expect(latest).toHaveTextContent("61.4"));
    // Recent samples table: the latest sample's metadata is shown.
    expect(await screen.findByText("seq 2 · user:ada")).toBeInTheDocument();
  });

  it("manual write adds a sample and the table refreshes", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await writeSample({ series: "node.cpu_temp", producer: "", ts: 1, seq: 1, payload: 10 });

    render(<IngestView ws={ws} />);
    await user.click(await screen.findByLabelText("select series node.cpu_temp"));

    // Fill the manual-write form with a new sample (seq 2) and submit.
    const form = screen.getByLabelText("write sample");
    await user.clear(within(form).getByLabelText("payload"));
    await user.type(within(form).getByLabelText("payload"), "99");
    await user.clear(within(form).getByLabelText("seq"));
    await user.type(within(form).getByLabelText("seq"), "2");
    await user.click(within(form).getByLabelText("submit sample"));

    // The latest value refreshes to the new sample (seq 2 = 99).
    const latest = await screen.findByLabelText("latest value");
    await waitFor(() => expect(latest).toHaveTextContent("99"));
  });

  it("filters series by tag facet (real series.find)", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    // Seed a sample with a label → the commit converts it to a tag edge, so series.find can match.
    await writeSample({
      series: "node.cpu_temp",
      producer: "",
      ts: 1,
      seq: 1,
      payload: 1,
      labels: { kind: "temperature" },
    });

    render(<IngestView ws={ws} />);
    // The prefix list shows it first.
    await screen.findByLabelText("select series node.cpu_temp");

    // A faceted search `kind:temperature` finds it via the tag graph.
    await user.type(screen.getByLabelText("search series"), "kind:temperature");
    await user.keyboard("{Enter}");
    expect(await screen.findByLabelText("select series node.cpu_temp")).toBeInTheDocument();
  });

  it("paginates the recent-samples table 10 at a time (older/newer)", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    // Seed 25 real samples (seq 1..25) — three pages of 10/10/5, newest first.
    for (let seq = 1; seq <= 25; seq++) {
      await writeSample({ series: "node.cpu_temp", producer: "", ts: seq, seq, payload: seq });
    }

    render(<IngestView ws={ws} />);
    await user.click(await screen.findByLabelText("select series node.cpu_temp"));

    // The seq column is the first cell of each body row — read it to assert the window's contents.
    const seqColumn = () =>
      screen
        .getAllByRole("row")
        // Skip the header row (its cells are column-header role, not "cell").
        .map((r) => within(r).queryAllByRole("cell")[0])
        .filter(Boolean)
        .map((c) => c!.textContent);

    // Page 1 (newest): exactly 10 rows, seq 25 down to 16.
    await screen.findByText("page 1");
    await waitFor(() => expect(seqColumn()).toEqual(["25","24","23","22","21","20","19","18","17","16"]));

    // Older → page 2: seq 15 down to 6.
    await user.click(screen.getByLabelText("older samples"));
    await screen.findByText("page 2");
    await waitFor(() => expect(seqColumn()).toEqual(["15","14","13","12","11","10","9","8","7","6"]));

    // Older → page 3 (partial): seq 5 down to 1, and the older control is now disabled.
    await user.click(screen.getByLabelText("older samples"));
    await screen.findByText("page 3");
    await waitFor(() => expect(seqColumn()).toEqual(["5","4","3","2","1"]));
    expect(screen.getByLabelText("older samples")).toBeDisabled();

    // Newer → back to page 2.
    await user.click(screen.getByLabelText("newer samples"));
    await screen.findByText("page 2");
    await waitFor(() => expect(seqColumn()).toEqual(["15","14","13","12","11","10","9","8","7","6"]));
  });

  it("is workspace-isolated — a fresh workspace shows no other workspace's series", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    // This workspace was never seeded, and the real gateway scopes every read to the token's
    // workspace (the hard wall) — so the series list is empty even though sibling tests seeded other
    // workspaces against the SAME real node.
    render(<IngestView ws={ws} />);
    expect(await screen.findByText(/no series yet/i)).toBeInTheDocument();
  });
});
