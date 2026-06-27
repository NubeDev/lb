import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { App } from "@/App";
import { rejectingBridge, stubBridge } from "@/test/bridge.stub";

// The page reaches real platform data ONLY through the bridge. These tests pass the bridge INTERFACE
// the shell provides (test double, not a fake node — testing-scope §0) with the REAL verb shapes
// (`series.find` → `{ series: string[] }`, `series.latest` → `{ sample }`), and prove the page lists
// the series the bridge returns, shows the selected series' latest value, and surfaces a denied /
// out-of-scope call as an HONEST error — never a fabricated list or value. The end-to-end proof against
// a REAL spawned gateway lives in `ui/src/features/ext-host/ProofPanel.gateway.test.tsx`.

describe("Panel", () => {
  it("starts idle, then lists the series series.find returns for a facet search", async () => {
    const user = userEvent.setup();
    const bridge = stubBridge({
      "series.find": () => ({ series: ["edge.temp", "edge.power"] }),
    });
    render(<App ctx={{ workspace: "acme" }} bridge={bridge} />);

    // Idle until the user constrains the query (empty series.find returns nothing — honest prompt).
    expect(screen.getByText(/Search a tag facet to list/i)).toBeInTheDocument();
    expect(screen.getByLabelText("workspace")).toHaveTextContent("acme");

    await user.type(screen.getByLabelText("series facet"), "kind:temperature");
    await user.click(screen.getByLabelText("run search"));

    expect(await screen.findByText("edge.temp")).toBeInTheDocument();
    expect(screen.getByText("edge.power")).toBeInTheDocument();
    // The bridge was called with the REAL host arg shape: a facets array, not `{ tags }`.
    expect(bridge.call).toHaveBeenCalledWith("series.find", {
      facets: [{ key: "kind", value: "temperature" }],
    });
  });

  it("shows the latest value of a selected series via series.latest", async () => {
    const user = userEvent.setup();
    const bridge = stubBridge({
      "series.find": () => ({ series: ["edge.temp"] }),
      "series.latest": () => ({ sample: { series: "edge.temp", seq: 7, payload: 61.4 } }),
    });
    render(<App ctx={{ workspace: "acme" }} bridge={bridge} />);

    await user.type(screen.getByLabelText("series facet"), "kind:temperature");
    await user.click(screen.getByLabelText("run search"));
    await user.click(await screen.findByLabelText("select edge.temp"));

    const latest = await screen.findByTestId("latest-payload");
    expect(latest).toHaveTextContent("61.4");
    expect(bridge.call).toHaveBeenCalledWith("series.latest", { series: "edge.temp" });
  });

  it("renders an honest empty state when the query matches no series", async () => {
    const user = userEvent.setup();
    const bridge = stubBridge({ "series.find": () => ({ series: [] }) });
    render(<App ctx={{ workspace: "acme" }} bridge={bridge} />);

    await user.type(screen.getByLabelText("series facet"), "kind:nothing");
    await user.click(screen.getByLabelText("run search"));

    expect(await screen.findByText(/No series match this query/i)).toBeInTheDocument();
  });

  it("surfaces a denied/out-of-scope find as an error, not a fabricated list", async () => {
    const user = userEvent.setup();
    const bridge = rejectingBridge("out_of_scope: series.find");
    render(<App ctx={{ workspace: "acme" }} bridge={bridge} />);

    await user.type(screen.getByLabelText("series facet"), "kind:temperature");
    await user.click(screen.getByLabelText("run search"));

    await waitFor(() =>
      expect(screen.getByText(/Could not load series: out_of_scope/i)).toBeInTheDocument(),
    );
  });

  it("surfaces a denied series.latest (grant-intersection narrowed) as an honest error", async () => {
    // The page is granted series.find but NOT series.latest (the admin approval narrowed it). The list
    // works; selecting a series and calling the ungranted verb is denied at the bridge — and the page
    // shows the error, never a blank or a fabricated value. This mirrors the real grant-intersection
    // deny path asserted end-to-end in the gateway test + the Rust host test.
    const user = userEvent.setup();
    const bridge = stubBridge({
      "series.find": () => ({ series: ["edge.temp"] }),
      // series.latest intentionally absent → the stub rejects it `out_of_scope`.
    });
    render(<App ctx={{ workspace: "acme" }} bridge={bridge} />);

    await user.type(screen.getByLabelText("series facet"), "kind:temperature");
    await user.click(screen.getByLabelText("run search"));
    await user.click(await screen.findByLabelText("select edge.temp"));

    expect(await screen.findByText(/Could not read latest: out_of_scope/i)).toBeInTheDocument();
    expect(screen.queryByTestId("latest-payload")).not.toBeInTheDocument();
  });
});
