// AgentView is unit-tested (not real-gateway) because agent_invoke needs a real model provider the
// gateway does not mock — documented deferral (the gateway has no /agent route; see
// src/lib/agent/agent.api.ts and the workflow/data real-gateway tests for the spawned-node pattern).
//
// So this is a PURE UNIT test of AgentView's rendering + wiring: we mock the view's OWN data hook
// (`./useAgent`) — not a backend fake — to drive each render branch (answer / denied / empty) and to
// assert the form invokes `run` with the typed goal. No `@/lib/ipc/*.fake` is imported.

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { AgentView } from "./AgentView";
import type { AgentState } from "./useAgent";

// Mock the hook module; each test sets the return value before rendering.
const run = vi.fn();
let state: AgentState;
vi.mock("./useAgent", () => ({
  useAgent: () => state,
}));

function setState(over: Partial<AgentState>) {
  state = { result: null, running: false, error: null, run, ...over };
}

afterEach(() => {
  cleanup();
  run.mockReset();
});

describe("AgentView (unit)", () => {
  it("renders the agent's answer when the invoke succeeds", () => {
    setState({ result: { answer: "agent: summarize the spec", jobId: "s1" } });
    render(<AgentView ws="acme" jobId="s1" author="user:ada" caps={[]} />);
    expect(screen.getByText("agent: summarize the spec")).toBeInTheDocument();
  });

  it("surfaces the node's denial to the user (the invoke gate)", () => {
    setState({ error: "denied" });
    render(<AgentView ws="acme" jobId="s1" author="user:cleo" caps={[]} />);
    expect(screen.getByRole("alert")).toHaveTextContent("You don't have access to the agent.");
  });

  it("prompts when there is no result yet", () => {
    setState({});
    render(<AgentView ws="acme" jobId="s1" author="user:ada" caps={[]} />);
    expect(screen.getByText("Ask the agent something.")).toBeInTheDocument();
  });

  it("submitting the goal invokes run with the trimmed goal", async () => {
    setState({});
    render(<AgentView ws="acme" jobId="s1" author="user:ada" caps={[]} />);
    await userEvent.type(screen.getByLabelText("goal"), "summarize the spec");
    await userEvent.click(screen.getByRole("button", { name: "Run" }));
    expect(run).toHaveBeenCalledWith("summarize the spec");
  });

  it("disables Run while a turn is in flight", () => {
    setState({ running: true });
    render(<AgentView ws="acme" jobId="s1" author="user:ada" caps={[]} />);
    expect(screen.getByRole("button", { name: "Running…" })).toBeDisabled();
  });
});
