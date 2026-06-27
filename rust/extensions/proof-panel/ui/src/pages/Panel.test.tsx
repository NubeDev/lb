import { render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { App } from "@/App";
import { rejectingBridge, stubBridge } from "@/test/bridge.stub";

// The page reaches real platform data ONLY through the bridge. These tests pass the bridge INTERFACE
// the shell provides (test double, not a fake node — testing-scope §0) and prove the page renders the
// rows the bridge returns and surfaces a denied/out-of-scope call as an HONEST error, never a fake list.

describe("Panel", () => {
  it("lists the series the bridge returns", async () => {
    const bridge = stubBridge({
      "series.find": () => [{ name: "edge.temp" }, { name: "edge.power" }],
    });
    render(<App ctx={{ workspace: "acme" }} bridge={bridge} />);

    await waitFor(() => expect(screen.getAllByTestId("series-row")).toHaveLength(2));
    expect(screen.getByText("edge.temp")).toBeInTheDocument();
    expect(screen.getByText("edge.power")).toBeInTheDocument();
    expect(screen.getByText("acme")).toBeInTheDocument(); // workspace badge from host ctx
  });

  it("shows the honest empty state when the workspace has no series", async () => {
    const bridge = stubBridge({ "series.find": () => [] });
    render(<App ctx={{ workspace: "acme" }} bridge={bridge} />);

    await waitFor(() =>
      expect(screen.getByText(/No series in this workspace yet/i)).toBeInTheDocument(),
    );
    expect(screen.queryByTestId("series-row")).not.toBeInTheDocument();
  });

  it("surfaces a denied/out-of-scope bridge call as an error, not a fabricated list", async () => {
    const bridge = rejectingBridge("denied");
    render(<App ctx={{ workspace: "acme" }} bridge={bridge} />);

    await waitFor(() =>
      expect(screen.getByText(/Could not load series: denied/i)).toBeInTheDocument(),
    );
    expect(screen.queryByTestId("series-row")).not.toBeInTheDocument();
  });
});
