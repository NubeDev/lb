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
import { setAgentConfig } from "@/lib/agent/config.api";
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
// the command appear), the runtime-picker read, and channel pub/sub. Plus (Slice 4) the admin
// `agent.config.set` to SEED an active pick + `agent.def.list` so the picker can resolve its human
// label — both real MCP verbs, no seed route needed.
const AGENT_CAPS = [
  "mcp:tools.catalog:call",
  "mcp:agent.invoke:call",
  "mcp:agent.runtimes:call",
  "mcp:agent.config.set:call",
  "mcp:agent.def.list:call",
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

  it("untouched, the picker defaults to the workspace's Active pick and posts NO `runtime` field", async () => {
    const ws = nextWs();
    await signInWithCaps("user:me", ws, AGENT_CAPS);
    // Seed a REAL active pick (the node offers `default`; a real registry-validated write). The picker
    // must now read "Active — <label>" from `agent.runtimes.workspace_default` — no second fetch.
    await setAgentConfig({ default_runtime: "default" });
    const user = userEvent.setup();
    render(<ChannelView ws={ws} channel="general" author="user:me" now={fixedClock()} />);
    await screen.findByText(/no messages yet/i);

    // / → command menu; the agent command is offered (its gate is held). Accept the best.
    await user.type(screen.getByLabelText("message"), "/agent");
    await screen.findByRole("listbox", { name: "commands" });
    await user.keyboard("{Enter}");

    // The required `goal` field AND the runtime dropdown are BOTH present the instant the command is
    // picked — the runtime picker renders persistently, NOT gated behind committing `goal`.
    const goal = await screen.findByLabelText("goal");
    const runtime = (await screen.findByLabelText("runtime")) as HTMLSelectElement;

    // The Active option is selected on mount and maps to the EMPTY value — so an untouched picker sends
    // NO runtime and the host's fallback (→ the workspace's active pick) runs. It reads "Active — …"
    // from the real `workspace_default` label, NOT a hardcoded runtime literal (registry-driven, so a
    // future arg widget cannot silently reintroduce the pre-fill regression).
    await waitFor(() => {
      const active = Array.from(runtime.options).find((o) => o.value === "");
      expect(active?.textContent).toMatch(/^Active — /);
    });
    expect(runtime.value).toBe(""); // the Active entry — no explicit override selected

    // Type the goal but NEVER touch the dropdown (the regression class: the composer pre-filling a
    // runtime that outranks the workspace pick).
    await user.type(goal, "summarize the incident");
    expect((screen.getByLabelText("runtime") as HTMLSelectElement).value).toBe("");

    // Submit → a STRUCTURED kind:"agent" Item lands in real history — and it carries NO `runtime` field
    // (the empty pick omits it, so the backend fallback resolves the active pick). This is the guard
    // against the whole regression class, kept registry-driven (no runtime literal asserted).
    await user.click(screen.getByLabelText("send"));
    let agentBody = "";
    await waitFor(async () => {
      const hist = await history(ws, "general");
      const item = hist.find((i) => i.body.includes('"kind":"agent"'));
      expect(item).toBeDefined();
      agentBody = item!.body;
    });
    const payload = JSON.parse(agentBody) as Record<string, unknown>;
    expect(payload.kind).toBe("agent");
    expect("runtime" in payload).toBe(false); // NO runtime on the wire — the active pick wins at the host

    // Drive the queued run through the real host path; the durable answer posts back.
    await drainAgentRuns();
    render(<ChannelView ws={ws} channel="general" author="user:me" now={fixedClock()} />);
    // The AgentCard settles to a durable agent_result answer (the unconfigured node's real canned text).
    const card = await screen.findByLabelText("agent result", undefined, { timeout: 10_000 });
    expect(card).toBeInTheDocument();
    expect(card).toHaveTextContent(/no in-house model is configured|configured on this node/i);
  });
});
