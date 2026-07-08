// The flow toolbar — the consolidated operator controls (flow-ui-polish scope). Pure-path render
// tests over the presentational component: Deploy is enabled ONLY when dirty; Run morphs to Stop
// mid-run; the single Pause⇄Resume toggle tracks the run's suspended status.

import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

import { FlowToolbar, type FlowToolbarProps } from "./FlowToolbar";

function props(over: Partial<FlowToolbarProps> = {}): FlowToolbarProps {
  return {
    dirty: false,
    runActive: false,
    runStatus: null,
    onDeploy: vi.fn(),
    onRun: vi.fn(),
    onLifecycle: vi.fn(),
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

  it("idle: Run is shown, Stop/Pause are not (the morphing button, idle side)", () => {
    const p = props({ runActive: false });
    render(<FlowToolbar {...p} />);
    fireEvent.click(screen.getByLabelText("run flow"));
    expect(p.onRun).toHaveBeenCalledOnce();
    expect(screen.queryByLabelText("stop run")).toBeNull();
    expect(screen.queryByLabelText("suspend run")).toBeNull();
  });

  it("run active: Run morphs to Stop (cancel) and the Pause toggle appears", () => {
    const p = props({ runActive: true, runStatus: "running" });
    render(<FlowToolbar {...p} />);
    expect(screen.queryByLabelText("run flow")).toBeNull();
    fireEvent.click(screen.getByLabelText("stop run"));
    expect(p.onLifecycle).toHaveBeenCalledWith("cancel");
    fireEvent.click(screen.getByLabelText("suspend run"));
    expect(p.onLifecycle).toHaveBeenCalledWith("suspend");
    // ONE toggle: while not suspended there is no separate Resume button.
    expect(screen.queryByLabelText("resume run")).toBeNull();
  });

  it("suspended run: the toggle flips to Resume", () => {
    const p = props({ runActive: true, runStatus: "suspended" });
    render(<FlowToolbar {...p} />);
    expect(screen.queryByLabelText("suspend run")).toBeNull();
    fireEvent.click(screen.getByLabelText("resume run"));
    expect(p.onLifecycle).toHaveBeenCalledWith("resume");
  });
});
