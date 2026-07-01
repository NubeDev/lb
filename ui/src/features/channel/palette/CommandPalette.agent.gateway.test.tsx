// The in-channel agent as a first-class palette command, driven against a REAL spawned gateway (no
// fake — CLAUDE §9). This is the run-lifecycle #5 acceptance proof for the composer entry point:
//   - a member WITH `mcp:agent.invoke:call` sees the agent command in `/` (the descriptor name IS the
//     gate — the same `authorize_tool` the run runs); one WITHOUT does NOT (absent, no existence leak);
//   - accepting it renders a runtime dropdown (default preselected, fed by the real `agent.runtimes`
//     read verb) + a goal field; submit posts a STRUCTURED `kind:"agent"` Item (NOT a raw agent.invoke
//     tool call) into real history;
//   - the run is driven through the real host path (`drain_channel_agent_runs`, the reactor's own
//     function) and the AgentCard settles to a durable `agent_result` answer — the default runtime over
//     the unconfigured model returns a real canned answer, so no model provider is needed.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { ChannelView } from "../ChannelView";
import { history } from "@/lib/channel/channel.api";
import {
  useRealGateway,
  signInWithCaps,
  drainAgentRuns,
} from "@/test/gateway-session";

let n = 0;
const nextWs = () => `palette-agent-${n++}`;

function fixedClock() {
  let t = 0;
  return () => ++t;
}

// The member cap set the agent palette command needs: the catalog read, the run gate (which also makes
// the command appear), the runtime-picker read, and channel pub/sub.
const AGENT_CAPS = [
  "mcp:tools.catalog:call",
  "mcp:agent.invoke:call",
  "mcp:agent.runtimes:call",
  "bus:chan/general:pub",
  "bus:chan/general:sub",
];

beforeAll(() => useRealGateway());

describe("CommandPalette — the agent command (real gateway)", () => {
  it("is capability-filtered: no `mcp:agent.invoke:call` → no agent command in the palette", async () => {
    const ws = nextWs();
    // Catalog + pub/sub but NOT the invoke gate → the command is absent (not greyed).
    await signInWithCaps("user:bob", ws, [
      "mcp:tools.catalog:call",
      "mcp:agent.runtimes:call",
      "bus:chan/general:pub",
      "bus:chan/general:sub",
    ]);
    const user = userEvent.setup();
    render(<ChannelView ws={ws} channel="general" author="user:bob" now={fixedClock()} />);
    await screen.findByText(/no messages yet/i);

    await user.type(screen.getByLabelText("message"), "/agent");
    await screen.findByRole("listbox", { name: "commands" });
    // No agent command offered (the run's gate is the catalog's gate).
    expect(screen.queryByText(/in-channel agent/i)).not.toBeInTheDocument();
  });

  it("accepts the agent command, renders the runtime dropdown + goal, and settles to an answer", async () => {
    const ws = nextWs();
    await signInWithCaps("user:me", ws, AGENT_CAPS);
    const user = userEvent.setup();
    render(<ChannelView ws={ws} channel="general" author="user:me" now={fixedClock()} />);
    await screen.findByText(/no messages yet/i);

    // / → command menu; the agent command is offered (its gate is held). Accept the best.
    await user.type(screen.getByLabelText("message"), "/agent");
    await screen.findByRole("listbox", { name: "commands" });
    await user.keyboard("{Enter}");

    // The arg rail opens on the required `goal` text field; type a goal.
    const goal = await screen.findByLabelText("goal");
    await user.type(goal, "summarize the incident");
    await user.keyboard("{Enter}"); // commit goal → the runtime dropdown takes focus

    // The runtime dropdown renders, fed by the real `agent.runtimes` verb, default preselected.
    const runtime = (await screen.findByLabelText("runtime")) as HTMLSelectElement;
    await waitFor(() => expect(runtime.value).toBe("default"));

    // Submit → a STRUCTURED kind:"agent" Item lands in real history (no raw `/`-text, no tool call).
    await user.click(screen.getByLabelText("send"));
    await waitFor(async () => {
      const hist = await history(ws, "general");
      expect(hist.some((i) => i.body.includes('"kind":"agent"'))).toBe(true);
    });

    // Drive the queued run through the real host path; the durable answer posts back.
    await drainAgentRuns();
    render(<ChannelView ws={ws} channel="general" author="user:me" now={fixedClock()} />);
    // The AgentCard settles to a durable agent_result answer (the unconfigured node's real canned text).
    const card = await screen.findByLabelText("agent result", undefined, { timeout: 10_000 });
    expect(card).toBeInTheDocument();
    expect(card).toHaveTextContent(/no in-house model is configured|configured on this node/i);
  });
});
