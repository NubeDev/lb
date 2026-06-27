import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { App } from "@/App";
import { stubBridge } from "@/test/bridge.stub";

// The page reaches real platform data ONLY through the bridge. These tests pass the bridge INTERFACE
// the shell provides (a test double, not a fake node — testing-scope §0) with the REAL verb shapes, and
// prove the "whole platform, one page" demo: the ingest→read round-trip (the page CREATES its data),
// the outbox status card, the inbox triage write, and the original series browse — each surfacing
// HONEST loading/empty/error/data states. The end-to-end proof against a REAL spawned gateway lives in
// `ui/src/features/ext-host/ProofPanel.gateway.test.tsx`; the load-bearing host gate is proven in
// rust/crates/host/tests/proof_panel_test.rs.

type Resolver = (a?: Record<string, unknown>) => unknown;

/** A bridge with sane empty defaults for the auto-loading verbs, so a section under test isn't noise. */
function demoBridge(overrides: Record<string, Resolver> = {}) {
  return stubBridge({
    "outbox.status": () => ({ pending: [], delivered: [], dead_lettered: [] }),
    "inbox.list": () => ({ items: [] }),
    "series.find": () => ({ series: [] }),
    "series.latest": () => ({ sample: null }),
    ...overrides,
  });
}

describe("Panel — the all-features demo", () => {
  it("renders the header with the workspace badge from the host ctx", () => {
    render(<App ctx={{ workspace: "acme" }} bridge={demoBridge()} />);
    expect(screen.getByRole("heading", { name: "Proof Panel" })).toBeInTheDocument();
    expect(screen.getByLabelText("workspace")).toHaveTextContent("acme");
  });

  it("ingest → read round-trip: Write sample calls ingest.write then re-reads series.latest", async () => {
    const user = userEvent.setup();
    // The latest read returns null first, then the just-written value after the write (so the page
    // shows write → read live). We flip the resolver after the write lands.
    let written = false;
    const bridge = demoBridge({
      "ingest.write": () => {
        written = true;
        return { accepted: 1 };
      },
      "series.latest": () => (written ? { sample: { seq: 1, payload: 21 } } : { sample: null }),
    });
    render(<App ctx={{ workspace: "acme" }} bridge={bridge} />);

    await user.click(screen.getByLabelText("write sample"));

    // The page wrote a real `Sample` shape (producer forced empty; the host stamps the real principal).
    await waitFor(() =>
      expect(bridge.call).toHaveBeenCalledWith(
        "ingest.write",
        expect.objectContaining({
          samples: [expect.objectContaining({ series: "proof.demo", seq: 1, payload: 21 })],
        }),
      ),
    );
    // And the read-back rendered the value it just wrote.
    expect(await screen.findByTestId("demo-latest")).toHaveTextContent("value 21");
  });

  it("ingest write denied → honest error, no fabricated value", async () => {
    const user = userEvent.setup();
    // ingest.write absent from the stub → rejected `out_of_scope`.
    const bridge = demoBridge();
    render(<App ctx={{ workspace: "acme" }} bridge={bridge} />);

    await user.click(screen.getByLabelText("write sample"));
    expect(await screen.findByText(/Could not write: out_of_scope/i)).toBeInTheDocument();
  });

  it("host-callback derive: Run derive calls proof.derive then re-reads proof.derived", async () => {
    const user = userEvent.setup();
    // The guest's `proof.derive` returns the value it committed; the read-back of `proof.derived`
    // shows it after the call lands. We flip the latest resolver once derive runs.
    let derived = false;
    const bridge = demoBridge({
      "proof-panel.proof.derive": () => {
        derived = true;
        return { derived: 42, source_seq: 7 };
      },
      "series.latest": (a) =>
        a?.series === "proof.derived" && derived ? { sample: { seq: 7, payload: 42 } } : { sample: null },
    });
    render(<App ctx={{ workspace: "acme" }} bridge={bridge} />);

    await user.click(screen.getByLabelText("run derive"));

    // The page invoked the EXTENSION's own backend tool (not a host-native verb).
    await waitFor(() =>
      expect(bridge.call).toHaveBeenCalledWith("proof-panel.proof.derive", {}),
    );
    expect(await screen.findByTestId("derive-result")).toHaveTextContent("Derived 42 from seq 7");
    // And the committed derived series read back through the bridge.
    expect(await screen.findByTestId("derived-latest")).toHaveTextContent("value 42");
  });

  it("host-callback derive denied → honest error, no fabricated value", async () => {
    const user = userEvent.setup();
    // proof.derive absent from the stub → rejected `out_of_scope` (the deny path).
    render(<App ctx={{ workspace: "acme" }} bridge={demoBridge()} />);
    await user.click(screen.getByLabelText("run derive"));
    expect(await screen.findByText(/Could not derive: out_of_scope/i)).toBeInTheDocument();
  });

  it("workflow simulation: Run produces an inbox item + outbox effect the sections then SHOW", async () => {
    const user = userEvent.setup();
    // The guest sim PRODUCES motion: before the click the inbox/outbox are empty; after it, the
    // produced item + effect appear (the resolvers flip once simulate runs — proving the page refreshes
    // the sections after the guest produces motion, the "I can finally see it work" payoff).
    let simulated = false;
    const bridge = demoBridge({
      "proof-panel.proof.simulate": () => {
        simulated = true;
        return { inbox_id: "proof-sim-item", resolved: true, outbox_pending: 1 };
      },
      "inbox.list": () =>
        simulated
          ? {
              items: [
                {
                  id: "proof-sim-item",
                  channel: "proof-triage",
                  author: "ext:proof-panel",
                  body: "proof.simulate: please approve this simulated request",
                  ts: 1,
                },
              ],
            }
          : { items: [] },
      "outbox.status": () =>
        simulated
          ? { pending: [{ id: "proof-sim-effect", target: "demo" }], delivered: [], dead_lettered: [] }
          : { pending: [], delivered: [], dead_lettered: [] },
    });
    render(<App ctx={{ workspace: "acme" }} bridge={bridge} />);

    // Empty before: the inbox shows its honest empty state.
    expect(await screen.findByTestId("inbox-empty")).toBeInTheDocument();

    await user.click(screen.getByLabelText("run workflow simulation"));

    // The page invoked the EXTENSION's own sim tool (not a host-native verb).
    await waitFor(() =>
      expect(bridge.call).toHaveBeenCalledWith("proof-panel.proof.simulate", {}),
    );
    // The summary shows each step landed.
    expect(await screen.findByTestId("simulate-result")).toHaveTextContent("proof-sim-item");

    // And the sections refreshed: the produced inbox item + outbox effect are now VISIBLE.
    expect(await screen.findByTestId("inbox-list")).toHaveTextContent(/please approve/i);
    await waitFor(() => expect(screen.getByTestId("outbox-pending")).toHaveTextContent("1"));
  });

  it("workflow simulation denied → honest error, no fabricated summary", async () => {
    const user = userEvent.setup();
    // proof.simulate absent from the stub → rejected `out_of_scope` (the deny path).
    render(<App ctx={{ workspace: "acme" }} bridge={demoBridge()} />);
    await user.click(screen.getByLabelText("run workflow simulation"));
    expect(await screen.findByTestId("simulate-error")).toHaveTextContent(/out_of_scope/i);
  });

  it("outbox status: renders the lifecycle counts and Refresh re-reads", async () => {
    const user = userEvent.setup();
    const bridge = demoBridge({
      "outbox.status": () => ({
        pending: [{ id: "e1" }],
        delivered: [{ id: "e2" }, { id: "e3" }],
        dead_lettered: [],
      }),
    });
    render(<App ctx={{ workspace: "acme" }} bridge={bridge} />);

    expect(await screen.findByTestId("outbox-pending")).toHaveTextContent("1");
    expect(screen.getByTestId("outbox-delivered")).toHaveTextContent("2");
    expect(screen.getByTestId("outbox-dead")).toHaveTextContent("0");

    await user.click(screen.getByLabelText("refresh outbox"));
    await waitFor(() =>
      expect(
        (bridge.call as ReturnType<typeof vi.fn>).mock.calls.filter((c) => c[0] === "outbox.status")
          .length,
      ).toBeGreaterThanOrEqual(2),
    );
  });

  it("inbox triage: lists items and Approve writes a resolution via inbox.resolve", async () => {
    const user = userEvent.setup();
    const bridge = demoBridge({
      "inbox.list": () => ({
        items: [{ id: "i1", channel: "triage", author: "ext:demo", body: "please review", ts: 1 }],
      }),
      "inbox.resolve": () => ({ ok: true }),
    });
    render(<App ctx={{ workspace: "acme" }} bridge={bridge} />);

    const list = await screen.findByTestId("inbox-list");
    expect(within(list).getByText("please review")).toBeInTheDocument();

    await user.click(screen.getByLabelText("approve i1"));
    await waitFor(() =>
      expect(bridge.call).toHaveBeenCalledWith(
        "inbox.resolve",
        expect.objectContaining({ item_id: "i1", decision: "approved" }),
      ),
    );
  });

  it("inbox triage: honest empty state when the channel has no items", async () => {
    render(<App ctx={{ workspace: "acme" }} bridge={demoBridge()} />);
    expect(await screen.findByTestId("inbox-empty")).toBeInTheDocument();
  });

  it("series browse: search lists the series series.find returns, select shows latest", async () => {
    const user = userEvent.setup();
    const bridge = demoBridge({
      "series.find": () => ({ series: ["edge.temp"] }),
      "series.latest": (a) =>
        a?.series === "edge.temp" ? { sample: { seq: 7, payload: 61.4 } } : { sample: null },
    });
    render(<App ctx={{ workspace: "acme" }} bridge={bridge} />);

    await user.type(screen.getByLabelText("series facet"), "kind:temperature");
    await user.click(screen.getByLabelText("run search"));
    await user.click(await screen.findByLabelText("select edge.temp"));

    expect(await screen.findByTestId("latest-payload")).toHaveTextContent("61.4");
  });
});
