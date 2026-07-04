// The runtime-state banner — the answer to "is this flow running?" on open. Purely INFORMATIONAL
// (flow-deploy-ux scope): running vs stopped, the schedule, the run count. Enable/Disable lives in the
// toolbar, so the banner owns no control — these are pure-path render tests over the presentational
// component; the running truth it shows comes from `runtimeState` (tested separately).

import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";

import { FlowRuntimeBanner } from "./FlowRuntimeBanner";
import type { FlowRuntimeState } from "./runtimeState";

function runtime(over: Partial<FlowRuntimeState>): FlowRuntimeState {
  return { state: "running", running: true, cron: "* * * * *", nextFireTs: null, latestRun: null, ...over };
}

describe("FlowRuntimeBanner (informational)", () => {
  it("a RUNNING flow shows 'Running' + the run count", () => {
    render(<FlowRuntimeBanner runtime={runtime({})} nowSecs={0} runCount={3} />);
    expect(screen.getByText(/^Running/i)).toBeTruthy();
    expect(screen.getByLabelText("run count").textContent).toContain("3");
  });

  it("a STOPPED (disabled) flow shows 'Stopped'", () => {
    render(<FlowRuntimeBanner runtime={runtime({ state: "stopped", running: false })} nowSecs={0} runCount={3} />);
    expect(screen.getByText(/stopped/i)).toBeTruthy();
  });

  it("a RUNNING flow with no self-firing source still renders the banner (no schedule detail)", () => {
    // Unlike the old model, a manual/flipflop flow is NOT hidden — it's a live runtime like any other.
    const { container } = render(
      <FlowRuntimeBanner runtime={runtime({ cron: null })} nowSecs={0} runCount={0} />,
    );
    expect(container.firstChild).not.toBeNull();
    expect(screen.getByText(/^Running/i)).toBeTruthy();
    expect(screen.queryByText(/schedule/i)).toBeNull();
  });

  it("owns no Enable/Disable control (it lives in the toolbar)", () => {
    render(<FlowRuntimeBanner runtime={runtime({})} nowSecs={0} runCount={0} />);
    expect(screen.queryByLabelText("disable flow")).toBeNull();
    expect(screen.queryByLabelText("deploy flow")).toBeNull();
  });
});
