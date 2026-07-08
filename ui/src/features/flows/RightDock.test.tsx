// The right dock (flow-ui-polish scope) — the single-instance invariant that motivated it: Config
// and Debug NEVER co-render (they were two side-by-side panels before); tab switching swaps content;
// close collapses the one dock.

import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

import { RightDock, type DockTab } from "./RightDock";
import { useState } from "react";

function Host({ initial = "config" as DockTab }) {
  const [tab, setTab] = useState<DockTab | null>(initial);
  if (!tab) return <div aria-label="dock closed" />;
  return (
    <RightDock
      tab={tab}
      onTabChange={setTab}
      onClose={() => setTab(null)}
      config={<div aria-label="config content">CONFIG</div>}
      debug={<div aria-label="debug content">DEBUG</div>}
    />
  );
}

describe("RightDock", () => {
  it("renders ONLY the active tab's content — config and debug never co-render", () => {
    render(<Host initial="config" />);
    expect(screen.getByLabelText("config content")).toBeTruthy();
    expect(screen.queryByLabelText("debug content")).toBeNull();
  });

  it("switching tabs swaps the content in the same dock", () => {
    render(<Host initial="config" />);
    fireEvent.click(screen.getByRole("tab", { name: "Debug" }));
    expect(screen.getByLabelText("debug content")).toBeTruthy();
    expect(screen.queryByLabelText("config content")).toBeNull();
    fireEvent.click(screen.getByRole("tab", { name: "Config" }));
    expect(screen.getByLabelText("config content")).toBeTruthy();
  });

  it("close collapses the dock entirely", () => {
    render(<Host initial="debug" />);
    fireEvent.click(screen.getByLabelText("close dock"));
    expect(screen.getByLabelText("dock closed")).toBeTruthy();
    expect(screen.queryByLabelText("flow right dock")).toBeNull();
  });

  it("the resize separator is keyboard-adjustable (accessible resize)", () => {
    render(<Host />);
    const sep = screen.getByLabelText("resize dock");
    const before = Number(sep.getAttribute("aria-valuenow"));
    fireEvent.keyDown(sep, { key: "ArrowLeft" });
    expect(Number(sep.getAttribute("aria-valuenow"))).toBe(before + 16);
  });

  it("onTabChange is a controlled callback (the canvas owns which tab is open)", () => {
    const onTabChange = vi.fn();
    render(
      <RightDock
        tab="config"
        onTabChange={onTabChange}
        onClose={vi.fn()}
        config={<span />}
        debug={<span />}
      />,
    );
    fireEvent.click(screen.getByRole("tab", { name: "Debug" }));
    expect(onTabChange).toHaveBeenCalledWith("debug");
  });
});
