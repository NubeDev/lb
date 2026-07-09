// The flow canvas header — the consolidated chrome (flow-ui-polish scope). Proves the Debug toggle
// stays reachable, every relocated action still fires through the `⋯` overflow menu, and the
// safety-relevant disabled state stays VISIBLE as a badge even though its control moved.

import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

import { FlowCanvasHeader, type FlowCanvasHeaderProps } from "./FlowCanvasHeader";

function props(over: Partial<FlowCanvasHeaderProps> = {}): FlowCanvasHeaderProps {
  return {
    dirty: false,
    runActive: false,
    runStatus: null,
    enabled: true,
    liveValues: false,
    canUndo: false,
    saveError: null,
    runError: null,
    debugOpen: false,
    onDeploy: vi.fn(),
    onRun: vi.fn(),
    onLifecycle: vi.fn(),
    onToggleEnabled: vi.fn(),
    onToggleLiveValues: vi.fn(),
    onUndo: vi.fn(),
    onTransfer: vi.fn(),
    onDelete: vi.fn(),
    onToggleDebug: vi.fn(),
    ...over,
  };
}

describe("FlowCanvasHeader — debug toggle", () => {
  it("renders the Debug button in the header toolbar", () => {
    render(<FlowCanvasHeader {...props()} />);
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

describe("FlowCanvasHeader — overflow menu (flow-ui-polish)", () => {
  it("idle header shows only the primary controls; relocated actions live behind ⋯", () => {
    render(<FlowCanvasHeader {...props()} />);
    // Primary: Deploy, Run, Debug, ⋯ — and nothing else.
    expect(screen.getByLabelText("deploy flow")).toBeTruthy();
    expect(screen.getByLabelText("run flow")).toBeTruthy();
    expect(screen.getByLabelText("more flow actions")).toBeTruthy();
    // Relocated actions are NOT rendered until the menu opens.
    for (const label of ["disable flow", "undo", "export flow", "import flow", "delete flow"]) {
      expect(screen.queryByLabelText(label)).toBeNull();
    }
  });

  it("every relocated action still fires its callback from the menu", () => {
    const p = props({ canUndo: true });
    render(<FlowCanvasHeader {...p} />);
    const open = () => fireEvent.click(screen.getByLabelText("more flow actions"));

    open();
    fireEvent.click(screen.getByLabelText("disable flow"));
    expect(p.onToggleEnabled).toHaveBeenCalledOnce();

    // The live-values row is a toggle, not a pick — flipping it keeps the menu OPEN (by design),
    // so the next action clicks straight through without reopening.
    open();
    fireEvent.click(screen.getByLabelText("toggle live values"));
    expect(p.onToggleLiveValues).toHaveBeenCalledWith(true);

    fireEvent.click(screen.getByLabelText("undo"));
    expect(p.onUndo).toHaveBeenCalledOnce();

    open();
    fireEvent.click(screen.getByLabelText("export flow"));
    expect(p.onTransfer).toHaveBeenCalledWith("export");

    open();
    fireEvent.click(screen.getByLabelText("import flow"));
    expect(p.onTransfer).toHaveBeenCalledWith("import");

    open();
    fireEvent.click(screen.getByLabelText("delete flow"));
    expect(p.onDelete).toHaveBeenCalledOnce();
  });

  it("undo is disabled in the menu when the stack is empty", () => {
    render(<FlowCanvasHeader {...props({ canUndo: false })} />);
    fireEvent.click(screen.getByLabelText("more flow actions"));
    expect((screen.getByLabelText("undo") as HTMLButtonElement).disabled).toBe(true);
  });

  it("a disabled flow stays visible as a badge (safety state never hides behind the menu)", () => {
    const { rerender } = render(<FlowCanvasHeader {...props({ enabled: true })} />);
    expect(screen.queryByLabelText("flow disabled")).toBeNull();
    rerender(<FlowCanvasHeader {...props({ enabled: false })} />);
    expect(screen.getByLabelText("flow disabled")).toBeTruthy();
  });
});
