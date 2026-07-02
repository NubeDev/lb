// The flow toolbar — the Node-RED operator controls (flow-deploy-ux scope). Pure-path render tests
// over the presentational component: Deploy is enabled ONLY when dirty; Enable/Disable reflects the
// durable flag; the live-values switch fires its handler; Suspend/Resume/Stop appear only mid-run.

import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

import { FlowToolbar, type FlowToolbarProps } from "./FlowToolbar";

function props(over: Partial<FlowToolbarProps> = {}): FlowToolbarProps {
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

describe("FlowToolbar", () => {
  it("Deploy is DISABLED when the canvas is clean", () => {
    render(<FlowToolbar {...props({ dirty: false })} />);
    const deploy = screen.getByLabelText("deploy flow") as HTMLButtonElement;
    expect(deploy.disabled).toBe(true);
    expect(deploy.textContent).toContain("Deployed");
  });

  it("Deploy is ENABLED and labeled 'Deploy' when dirty, and fires onDeploy", () => {
    const p = props({ dirty: true });
    render(<FlowToolbar {...p} />);
    const deploy = screen.getByLabelText("deploy flow") as HTMLButtonElement;
    expect(deploy.disabled).toBe(false);
    expect(deploy.textContent).toContain("Deploy");
    fireEvent.click(deploy);
    expect(p.onDeploy).toHaveBeenCalledOnce();
  });

  it("shows Disable when enabled, Enable when disabled — and fires onToggleEnabled", () => {
    const p = props({ enabled: true });
    const { rerender } = render(<FlowToolbar {...p} />);
    expect(screen.getByLabelText("disable flow")).toBeTruthy();
    fireEvent.click(screen.getByLabelText("disable flow"));
    expect(p.onToggleEnabled).toHaveBeenCalledOnce();

    rerender(<FlowToolbar {...props({ enabled: false })} />);
    expect(screen.getByLabelText("enable flow")).toBeTruthy();
  });

  it("the live-values switch fires onToggleLiveValues", () => {
    const p = props({ liveValues: false });
    render(<FlowToolbar {...p} />);
    fireEvent.click(screen.getByLabelText("toggle live values"));
    expect(p.onToggleLiveValues).toHaveBeenCalledWith(true);
  });

  it("Suspend/Resume/Stop appear only while a run is active", () => {
    const { rerender } = render(<FlowToolbar {...props({ runActive: false })} />);
    expect(screen.queryByLabelText("stop run")).toBeNull();
    rerender(<FlowToolbar {...props({ runActive: true })} />);
    expect(screen.getByLabelText("suspend run")).toBeTruthy();
    expect(screen.getByLabelText("resume run")).toBeTruthy();
    expect(screen.getByLabelText("stop run")).toBeTruthy();
  });

  it("Run is disabled while a run is active (no double-start)", () => {
    render(<FlowToolbar {...props({ runActive: true })} />);
    expect((screen.getByLabelText("run flow") as HTMLButtonElement).disabled).toBe(true);
  });
});
