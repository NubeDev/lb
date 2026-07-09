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

  it("renders the stall pause-and-ask prompt with Keep going + Stop, and fires them", async () => {
    const user = userEvent.setup();
    const onKeepGoing = vi.fn();
    const onStopStalled = vi.fn();
    render(
      <DockRunStatus
        phase="working"
        feed={FEED}
        elapsedSec={95}
        degraded={false}
        stalled
        stalledText="The agent hasn't made progress for a while — it may be stuck."
        onRetry={() => {}}
        onKeepGoing={onKeepGoing}
        onStopStalled={onStopStalled}
      />,
    );
    // The honest prompt shows, and the run reads as awaiting a decision (not a bare spinner/error).
    expect(screen.getByLabelText("run stalled — awaiting your decision")).toBeInTheDocument();
    expect(screen.getByText(/may be stuck/)).toBeInTheDocument();
    await user.click(screen.getByLabelText("keep going"));
    await user.click(screen.getByLabelText("stop run"));
    expect(onKeepGoing).toHaveBeenCalledOnce();
    expect(onStopStalled).toHaveBeenCalledOnce();
  });

  it("stall prompt yields to a terminal Done/Error state (the decision was made)", () => {
    render(
      <DockRunStatus
        phase="done"
        feed={FEED}
        elapsedSec={95}
        degraded={false}
        stalled
        onRetry={() => {}}
        onKeepGoing={() => {}}
      />,
    );
    // Done wins — the stall prompt is gone once the run settled.
    expect(screen.queryByLabelText("run stalled — awaiting your decision")).toBeNull();
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

describe("DockRunStatus tool list", () => {
  const TOOLS: RunFeed = {
    live: true,
    text: "",
    reasoning: "",
    finished: false,
    tools: [
      { id: "c1", name: "datasource.list", ok: "{}", err: null },
      { id: "c2", name: "viz.query", err: "denied", ok: null },
      { id: "c3", name: "dashboard.pin" },
    ],
  };

  it("renders each tool call as a row while working (done, failed, running)", () => {
    render(
      <DockRunStatus phase="working" feed={TOOLS} elapsedSec={4} degraded={false} onRetry={() => {}} />,
    );
    const list = screen.getByLabelText("tool calls");
    expect(list).toHaveTextContent("datasource.list");
    expect(list).toHaveTextContent("viz.query");
    expect(list).toHaveTextContent("denied");
    expect(list).toHaveTextContent("dashboard.pin");
  });

  it("keeps the tool list visible after the run is done (the durable answer has no tool record)", () => {
    render(
      <DockRunStatus phase="done" feed={TOOLS} elapsedSec={9} degraded={false} onRetry={() => {}} />,
    );
    expect(screen.getByLabelText("tool calls")).toHaveTextContent("datasource.list");
  });

  it("renders nothing on done when no tools were called and not degraded", () => {
    const { container } = render(
      <DockRunStatus phase="done" feed={FEED} elapsedSec={9} degraded={false} onRetry={() => {}} />,
    );
    expect(container).toBeEmptyDOMElement();
  });
});

describe("DockRunStatus tool list FIFO cap", () => {
  // 7 tools — 2 over the MAX_VISIBLE_TOOLS (5) cap. The OLDEST two should be hidden first; the newest
  // 5 (incl. any in-flight tail) stay anchored at the bottom of the list where the eye lands.
  const MANY_TOOLS: RunFeed = {
    live: true,
    text: "",
    reasoning: "",
    finished: false,
    tools: [
      { id: "c1", name: "alpha.get" },
      { id: "c2", name: "beta.list", ok: "{}", err: null },
      { id: "c3", name: "gamma.run" },
      { id: "c4", name: "delta.save", ok: "{}", err: null },
      { id: "c5", name: "epsilon.query", ok: "{}", err: null },
      { id: "c6", name: "zeta.schema", ok: "{}", err: null },
      { id: "c7", name: "eta.run" },
    ],
  };

  it("FIFO-collapses: hides the OLDEST calls and shows the newest within the cap", () => {
    render(
      <DockRunStatus phase="working" feed={MANY_TOOLS} elapsedSec={4} degraded={false} onRetry={() => {}} />,
    );
    const list = screen.getByLabelText("tool calls");
    // Newest 5 visible (the tail of the run, where activity is happening).
    expect(list).toHaveTextContent("gamma.run");
    expect(list).toHaveTextContent("delta.save");
    expect(list).toHaveTextContent("epsilon.query");
    expect(list).toHaveTextContent("zeta.schema");
    expect(list).toHaveTextContent("eta.run");
    // Oldest two FIFO-evicted from the default view.
    expect(list).not.toHaveTextContent("alpha.get");
    expect(list).not.toHaveTextContent("beta.list");
    // The honest "hidden" affordance.
    expect(screen.getByText("2 earlier calls hidden")).toBeInTheDocument();
    expect(screen.getByLabelText("show all tool calls")).toBeInTheDocument();
  });

  it("Show all reveals every call in original order", async () => {
    const user = userEvent.setup();
    render(
      <DockRunStatus phase="working" feed={MANY_TOOLS} elapsedSec={4} degraded={false} onRetry={() => {}} />,
    );
    await user.click(screen.getByLabelText("show all tool calls"));
    const list = screen.getByLabelText("tool calls");
    // First-seen order preserved — the oldest re-appears at the top.
    expect(list).toHaveTextContent("alpha.get");
    expect(list).toHaveTextContent("beta.list");
    expect(list).toHaveTextContent("eta.run");
    expect(screen.queryByText("2 earlier calls hidden")).toBeNull();
    expect(screen.getByLabelText("show fewer tool calls")).toBeInTheDocument();
  });

  it("Show fewer re-collapses after an expand", async () => {
    const user = userEvent.setup();
    render(
      <DockRunStatus phase="working" feed={MANY_TOOLS} elapsedSec={4} degraded={false} onRetry={() => {}} />,
    );
    await user.click(screen.getByLabelText("show all tool calls"));
    await user.click(screen.getByLabelText("show fewer tool calls"));
    const list = screen.getByLabelText("tool calls");
    expect(list).not.toHaveTextContent("alpha.get");
    expect(screen.getByText("2 earlier calls hidden")).toBeInTheDocument();
  });

  it("does not collapse when the count is at or under the cap", () => {
    const at: RunFeed = { ...MANY_TOOLS, tools: MANY_TOOLS.tools.slice(0, 5) };
    render(<DockRunStatus phase="working" feed={at} elapsedSec={4} degraded={false} onRetry={() => {}} />);
    expect(screen.queryByText(/earlier calls hidden/)).toBeNull();
    expect(screen.queryByLabelText("show all tool calls")).toBeNull();
  });
});
