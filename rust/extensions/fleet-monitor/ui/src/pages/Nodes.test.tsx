import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";

import { Nodes } from "@/pages/Nodes";
import { BridgeContext } from "@/app/bridge-context";
import { rejectingBridge, stubBridge } from "@/test/bridge.stub";
import type { Bridge } from "@/app/contract";

function renderNodes(bridge: Bridge) {
  return render(
    <BridgeContext.Provider value={{ ctx: { workspace: "acme" }, bridge }}>
      <MemoryRouter>
        <Nodes />
      </MemoryRouter>
    </BridgeContext.Provider>,
  );
}

describe("Nodes", () => {
  it("calls series.find and renders the resolved list", async () => {
    const bridge = stubBridge({
      "series.find": () => [{ name: "node-a" }, { name: "node-b" }],
    });
    renderNodes(bridge);

    expect(await screen.findByText("node-a")).toBeInTheDocument();
    expect(screen.getByText("node-b")).toBeInTheDocument();
    expect(screen.getAllByTestId("node-row")).toHaveLength(2);
    expect(bridge.call).toHaveBeenCalledWith("series.find", { tags: [] });
  });

  it("renders an honest error state when the bridge call rejects (out of scope / denied)", async () => {
    renderNodes(rejectingBridge("out_of_scope: series.find"));
    expect(await screen.findByText(/Could not load series/)).toBeInTheDocument();
    expect(screen.getByText(/out_of_scope: series.find/)).toBeInTheDocument();
  });

  it("renders an honest empty state when the bridge returns no series", async () => {
    renderNodes(stubBridge({ "series.find": () => [] }));
    expect(await screen.findByText(/No series in this workspace/)).toBeInTheDocument();
  });
});
