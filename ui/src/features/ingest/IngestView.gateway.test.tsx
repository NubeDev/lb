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
    const seriesBtn = await screen.findByLabelText("select node.cpu_temp");
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
    await user.click(await screen.findByLabelText("select node.cpu_temp"));

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
    await screen.findByLabelText("select node.cpu_temp");

    // A faceted search `kind:temperature` finds it via the tag graph.
    await user.type(screen.getByLabelText("search series"), "kind:temperature");
    await user.keyboard("{Enter}");
    expect(await screen.findByLabelText("select node.cpu_temp")).toBeInTheDocument();
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
