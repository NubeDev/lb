// Unit tests for the dock run-status controls (agent-dock run controls) — the pause/stop/resume
// affordances render for the right states and fire their handlers. Presentation only (no gateway).

import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import type { RunFeed } from "@/features/channel/useRunFeed";
import { DockRunStatus } from "./DockRunStatus";

const FEED: RunFeed = { live: true, text: "", reasoning: "thinking", tools: [], finished: false };

describe("DockRunStatus controls", () => {
  it("shows Pause + Stop while a run is working, and fires them", async () => {
    const user = userEvent.setup();
    const onPause = vi.fn();
    const onStop = vi.fn();
    render(
      <DockRunStatus
        phase="working"
        feed={FEED}
        elapsedSec={3}
        degraded={false}
        onRetry={() => {}}
        onPause={onPause}
        onStop={onStop}
      />,
    );
    await user.click(screen.getByLabelText("pause run"));
    await user.click(screen.getByLabelText("stop run"));
    expect(onPause).toHaveBeenCalledOnce();
    expect(onStop).toHaveBeenCalledOnce();
  });

  it("shows Resume (not Pause/Stop) when paused, and fires it", async () => {
    const user = userEvent.setup();
    const onResume = vi.fn();
    render(
      <DockRunStatus
        phase="working"
        feed={FEED}
        elapsedSec={5}
        degraded={false}
        paused
        onRetry={() => {}}
        onResume={onResume}
      />,
    );
    expect(screen.getByLabelText("run paused")).toBeInTheDocument();
    expect(screen.queryByLabelText("pause run")).toBeNull();
    await user.click(screen.getByLabelText("resume run"));
    expect(onResume).toHaveBeenCalledOnce();
  });

  it("shows controls even in the pre-delta Sent state (the run may already be driving)", () => {
    render(
      <DockRunStatus
        phase="sent"
        feed={FEED}
        elapsedSec={0}
        degraded={false}
        onRetry={() => {}}
        onPause={() => {}}
        onStop={() => {}}
      />,
    );
    // A run can be paused/stopped the moment it's sent — the run job may already exist server-side.
    expect(screen.getByLabelText("pause run")).toBeInTheDocument();
    expect(screen.getByLabelText("stop run")).toBeInTheDocument();
  });

  it("hides controls entirely when no handlers are wired (no agent.control grant)", () => {
    render(
      <DockRunStatus phase="working" feed={FEED} elapsedSec={2} degraded={false} onRetry={() => {}} />,
    );
    expect(screen.queryByLabelText("pause run")).toBeNull();
    expect(screen.queryByLabelText("stop run")).toBeNull();
  });

  it("shows Retry in the error state", async () => {
    const user = userEvent.setup();
    const onRetry = vi.fn();
    render(
      <DockRunStatus
        phase="error"
        feed={FEED}
        elapsedSec={0}
        degraded={false}
        errorText="agent not permitted"
        onRetry={onRetry}
      />,
    );
    expect(screen.getByText("agent not permitted")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /retry/i }));
    expect(onRetry).toHaveBeenCalledOnce();
  });
});
