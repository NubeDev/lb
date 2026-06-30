// The runtime-state banner — the answer to "is this flow running?" on open, plus the durable
// Deploy/Stop control for a headless flow (the Stop the user couldn't find for a cron flow, which has
// no live run to cancel). Pure-PATH render tests over the presentational component; the enabled truth
// it shows comes from `armedState` (tested separately) — here we assert the banner renders the right
// label + fires onToggle.

import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

import { FlowArmedBanner } from "./FlowArmedBanner";
import type { FlowArmedState } from "./armedState";

function armed(over: Partial<FlowArmedState>): FlowArmedState {
  return { kind: "armed", scheduled: true, cron: "* * * * *", nextFireTs: null, latestRun: null, ...over };
}

describe("FlowArmedBanner Deploy/Stop toggle", () => {
  it("an ARMED flow shows 'running headless' + a Stop button that fires onToggle", () => {
    const onToggle = vi.fn();
    render(<FlowArmedBanner armed={armed({})} nowSecs={0} runCount={3} onToggle={onToggle} />);
    expect(screen.getByText(/running headless/i)).toBeTruthy();
    const stop = screen.getByLabelText("stop flow");
    fireEvent.click(stop);
    expect(onToggle).toHaveBeenCalledOnce();
  });

  it("a DISABLED flow shows 'nothing fires' + a Deploy button (the durable re-arm)", () => {
    const onToggle = vi.fn();
    render(
      <FlowArmedBanner armed={armed({ kind: "disabled" })} nowSecs={0} runCount={3} onToggle={onToggle} />,
    );
    expect(screen.getByText(/nothing fires/i)).toBeTruthy();
    const deploy = screen.getByLabelText("deploy flow");
    fireEvent.click(deploy);
    expect(onToggle).toHaveBeenCalledOnce();
  });

  it("an IDLE (manual) flow renders no banner at all", () => {
    const { container } = render(
      <FlowArmedBanner armed={armed({ kind: "idle", scheduled: false })} nowSecs={0} runCount={0} />,
    );
    expect(container.firstChild).toBeNull();
  });

  it("omitting onToggle hides the control (read-only banner)", () => {
    render(<FlowArmedBanner armed={armed({})} nowSecs={0} runCount={0} />);
    expect(screen.queryByLabelText("stop flow")).toBeNull();
  });
});
