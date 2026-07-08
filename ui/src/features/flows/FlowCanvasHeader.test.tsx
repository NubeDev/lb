// The flow canvas header — render test for the debug-panel toggle (debug-node-scope). Proves the Bug
// button is present in the header (the discoverability regression that forced this rework: the prior
// floating `absolute` button escaped the non-`relative` canvas and was invisible), that clicking it
// fires the toggle, and that the highlighted/pressed state reflects `debugOpen`. Presentational:
// every action + piece of state is a prop.

import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

import { FlowCanvasHeader, type FlowCanvasHeaderProps } from "./FlowCanvasHeader";
import type { FlowToolbarProps } from "./FlowToolbar";

function toolbarProps(over: Partial<FlowToolbarProps> = {}): FlowToolbarProps {
  return {
    dirty: false,
    runActive: false,
    enabled: true,
    liveValues: false,
    onDeploy: vi.fn(),
    onRun: vi.fn(),
    onLifecycle: vi.fn(),
    onToggleEnabled: vi.fn(),
    onToggleLiveValues: vi.fn(),
    ...over,
  };
}

function props(over: Partial<FlowCanvasHeaderProps> = {}): FlowCanvasHeaderProps {
  return {
    ...toolbarProps(),
    canUndo: false,
    runStatus: null,
    saveError: null,
    runError: null,
    debugOpen: false,
    onUndo: vi.fn(),
    onExport: vi.fn(),
    onImport: vi.fn(),
    onDelete: vi.fn(),
    onToggleDebug: vi.fn(),
    ...over,
  };
}

describe("FlowCanvasHeader — debug toggle", () => {
  it("renders the Debug button in the header toolbar", () => {
    render(<FlowCanvasHeader {...props()} />);
    // The button is reachable from the header (not a floating escapee).
    expect(screen.getByLabelText("open debug panel")).toBeTruthy();
    expect(screen.getByLabelText("open debug panel").textContent).toContain("Debug");
  });

  it("fires onToggleDebug when clicked", () => {
    const onToggleDebug = vi.fn();
    render(<FlowCanvasHeader {...props({ onToggleDebug })} />);
    fireEvent.click(screen.getByLabelText("open debug panel"));
    expect(onToggleDebug).toHaveBeenCalledOnce();
  });

  it("reflects the open state: aria-pressed + the close label", () => {
    render(<FlowCanvasHeader {...props({ debugOpen: true })} />);
    const btn = screen.getByLabelText("close debug panel");
    expect(btn).toBeTruthy();
    expect(btn.getAttribute("aria-pressed")).toBe("true");
  });
});
