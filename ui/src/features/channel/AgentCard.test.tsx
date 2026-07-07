// Unit tests for AgentCard (channels-agent scope). RENDER only — no gateway. The headline regression:
// a settled `agent` payload (its `agent_result`/`agent_error` already in the channel) still renders
// the user's GOAL as their chat turn. Returning null there wiped the user's own message the moment
// the agent replied — the "my message vanished" bug. Also locks: the durable result/error cards no
// longer echo the goal (it would double up now that the user's card carries it).

import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";

import { AgentCard } from "./AgentCard";
import type {
  AgentErrorPayload,
  AgentPayload,
  AgentResultPayload,
} from "@/lib/channel/payload.types";

const GOAL = "How do I scope a surreal record by workspace?";

const agent: AgentPayload = { kind: "agent", goal: GOAL, job: "run-1" };
const result: AgentResultPayload = {
  kind: "agent_result",
  goal: GOAL,
  runtime: "default",
  job: "run-1",
  answer: "Use the workspace id in the record id prefix.",
};
const failure: AgentErrorPayload = {
  kind: "agent_error",
  goal: GOAL,
  error: "no in-house model is configured on this node",
};

describe("AgentCard — settled agent payload keeps the user's goal visible (the bug)", () => {
  it("renders the goal when settled=true (was null — the user's message vanished)", () => {
    render(<AgentCard payload={agent} settled />);
    // The user's goal must be on screen — it is their only chat turn.
    expect(screen.getByText(GOAL)).toBeInTheDocument();
    // The live-run chrome (spinner, tool list) does NOT render in the settled state.
    expect(screen.queryByLabelText("agent tool calls")).toBeNull();
    expect(screen.queryByText("running…")).toBeNull();
  });

  it("renders the RunningCard (goal + running chip) when NOT settled", () => {
    render(<AgentCard payload={agent} />);
    expect(screen.getByText(GOAL)).toBeInTheDocument();
    expect(screen.getByText("running…")).toBeInTheDocument();
  });
});

describe("AgentCard — agent_result no longer echoes the goal", () => {
  it("renders the runtime chip + answer, but NOT the goal (the user's card above carries it)", () => {
    render(<AgentCard payload={result} />);
    expect(screen.getByLabelText("agent result")).toBeInTheDocument();
    expect(screen.getByText("in-house agent")).toBeInTheDocument();
    expect(screen.getByText(result.answer)).toBeInTheDocument();
    // The goal is intentionally NOT echoed here — the settled `agent` card above shows it.
    expect(screen.queryByText(GOAL)).toBeNull();
  });

  it("shows the truncation note when the answer was capped", () => {
    render(<AgentCard payload={{ ...result, truncated: true }} />);
    expect(screen.getByText(/answer truncated/i)).toBeInTheDocument();
  });
});

describe("AgentCard — agent_error surfaces the failure, not the goal", () => {
  it("renders the error message in an alert, without echoing the goal", () => {
    render(<AgentCard payload={failure} />);
    expect(screen.getByRole("alert")).toBeInTheDocument();
    expect(screen.getByText(failure.error)).toBeInTheDocument();
    // The goal is NOT echoed here either — the user's `agent` card above shows it.
    expect(screen.queryByText(GOAL)).toBeNull();
  });
});
