// The runtime-state banner — the answer to "is this flow running?" on open. Purely INFORMATIONAL
// (flow-deploy-ux scope): armed vs disabled, the schedule, the run count. Enable/Disable moved to the
// toolbar, so the banner owns no control — these are pure-path render tests over the presentational
// component; the enabled truth it shows comes from `armedState` (tested separately).

import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";

import { FlowArmedBanner } from "./FlowArmedBanner";
import type { FlowArmedState } from "./armedState";

function armed(over: Partial<FlowArmedState>): FlowArmedState {
  return { kind: "armed", scheduled: true, cron: "* * * * *", nextFireTs: null, latestRun: null, ...over };
}

describe("FlowArmedBanner (informational)", () => {
  it("an ARMED flow shows 'running headless' + the run count", () => {
    render(<FlowArmedBanner armed={armed({})} nowSecs={0} runCount={3} />);
    expect(screen.getByText(/running headless/i)).toBeTruthy();
    expect(screen.getByLabelText("run count").textContent).toContain("3");
  });

  it("a DISABLED flow shows 'nothing fires'", () => {
    render(<FlowArmedBanner armed={armed({ kind: "disabled" })} nowSecs={0} runCount={3} />);
    expect(screen.getByText(/nothing fires/i)).toBeTruthy();
  });

  it("an IDLE (manual) flow renders no banner at all", () => {
    const { container } = render(
      <FlowArmedBanner armed={armed({ kind: "idle", scheduled: false })} nowSecs={0} runCount={0} />,
    );
    expect(container.firstChild).toBeNull();
  });

  it("owns no Enable/Disable control (it moved to the toolbar)", () => {
    render(<FlowArmedBanner armed={armed({})} nowSecs={0} runCount={0} />);
    expect(screen.queryByLabelText("stop flow")).toBeNull();
    expect(screen.queryByLabelText("deploy flow")).toBeNull();
  });
});
