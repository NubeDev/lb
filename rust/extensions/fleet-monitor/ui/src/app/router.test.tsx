import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { App } from "@/App";
import { stubBridge } from "@/test/bridge.stub";

function renderApp() {
  const bridge = stubBridge({
    "series.find": () => [{ name: "node-a" }, { name: "node-b" }],
    "series.latest": () => ({ series: "node-a", value: 42 }),
  });
  render(<App ctx={{ workspace: "acme" }} bridge={bridge} />);
  return bridge;
}

describe("nested routing", () => {
  it("renders Overview parent with the sub-nav and the Nodes index child in the Outlet", async () => {
    renderApp();
    // Parent sub-nav present on the index route.
    expect(screen.getByRole("tab", { name: "Nodes" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Alerts" })).toBeInTheDocument();
    // Index child rendered.
    expect(await screen.findByText("Fleet Nodes")).toBeInTheDocument();
  });

  it("navigates to the Alerts nested child, keeping the parent sub-nav", async () => {
    renderApp();
    fireEvent.click(screen.getByRole("tab", { name: "Alerts" }));
    // Bridge probe surfaced honestly in the Alerts child (rendered in the parent Outlet).
    await waitFor(() => expect(screen.getByText(/Bridge connected/)).toBeInTheDocument());
    // Heading "Alerts" is the child's CardTitle (distinct from the tab).
    expect(screen.getByRole("heading", { name: /Alerts/ })).toBeInTheDocument();
    // Parent sub-nav still present (child renders inside the parent Outlet).
    expect(screen.getByRole("tab", { name: "Nodes" })).toBeInTheDocument();
  });
});
