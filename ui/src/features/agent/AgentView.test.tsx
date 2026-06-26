// The S5 invoke gate, at the UI level: a user WITH `mcp:agent.invoke:call` gets the agent's
// answer; a user WITHOUT it sees the node's "denied"; and an ungranted substrate skill is denied —
// the same gates the Rust `agent_test` proves on the backend, surfaced through the real api client
// + the faithful in-memory fake.
//
// We drive the actual `agent.api` → `invoke` → fake path (no mock of the api): the fake mirrors the
// node's invoke + grant gates, so this exercises the allow/deny branches the user actually hits.

import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { AgentView } from "./AgentView";
import { __grantAgentSkill, __resetAgentFake } from "@/lib/ipc/agent.fake";

const WS = "acme";
const INVOKE = "mcp:agent.invoke:call";

beforeEach(() => __resetAgentFake());
afterEach(() => __resetAgentFake());

describe("AgentView invoke gate", () => {
  it("a user with the invoke grant gets the agent's answer", async () => {
    render(<AgentView ws={WS} jobId="s1" author="user:ada" caps={[INVOKE]} />);
    await userEvent.type(screen.getByLabelText("goal"), "summarize the spec");
    await userEvent.click(screen.getByRole("button", { name: "Run" }));
    await waitFor(() =>
      expect(screen.getByText("agent: summarize the spec")).toBeInTheDocument(),
    );
  });

  it("a user WITHOUT the invoke grant is denied (the gate surfaced to the user)", async () => {
    render(<AgentView ws={WS} jobId="s1" author="user:cleo" caps={[]} />);
    await userEvent.type(screen.getByLabelText("goal"), "do a thing");
    await userEvent.click(screen.getByRole("button", { name: "Run" }));
    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent(
        "You don't have access to the agent.",
      ),
    );
  });

  it("invoking with a granted skill succeeds; an ungranted skill is denied", async () => {
    // A small wrapper to invoke with a skill: the AgentView form only sets the goal, so we drive
    // the gate through the hook's surface by granting (or not) the skill in the fake.
    __grantAgentSkill(WS, "summarize");
    const { invokeAgent } = await import("@/lib/agent/agent.api");

    const ok = await invokeAgent(WS, "s2", "go", {
      skill: "summarize",
      author: "user:ada",
      caps: [INVOKE],
    });
    expect(ok.answer).toBe("agent: go");

    // An ungranted skill is invisible to the agent → denied (the S4 grant gate it inherits).
    await expect(
      invokeAgent(WS, "s3", "go", { skill: "secret", author: "user:ada", caps: [INVOKE] }),
    ).rejects.toThrow("denied");
  });
});
