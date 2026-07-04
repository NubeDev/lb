// The agent-config follow-up (runtime-id resolution) proven END TO END over a REAL spawned gateway
// (no mocks, no fake backend — CLAUDE §9): an admin sets the workspace's default runtime in the
// Settings → Agent UI, and a subsequent channel `kind:"agent"` run with NO explicit runtime resolves
// to that stored default (explicit arg → workspace default → registry default).
//
// The test gateway is a default-only node (no `--features external-agent`), so the observable stored
// default here is `"default"` (the in-house loop). The DISTINGUISHING precedence — a stored EXTERNAL
// id actually selecting a different engine, and a stored-but-unavailable id falling back — is proven
// in the backend `agent_default_runtime_test.rs` against a registered stub runtime; over the gateway
// we prove the WIRING: the UI persists the choice, and an omitted-runtime run consults it and settles.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { SettingsHarness } from "./SettingsHarness";
import { getAgentConfig } from "@/lib/agent/config.api";
import { post, history } from "@/lib/channel/channel.api";
import type { Item } from "@/lib/channel/channel.types";
import {
  useRealGateway,
  signInWithCaps,
  drainAgentRuns,
} from "@/test/gateway-session";

let n = 0;
const nextWs = () => `agent-default-${n++}`;

// The caps an admin needs: set/read the agent config, run the agent, and pub/sub the channel the run
// posts its result into.
const ADMIN_CAPS = [
  "mcp:agent.config.get:call",
  "mcp:agent.config.set:call",
  "mcp:agent.runtimes:call",
  // agent-catalog scope: the Agent tab is now a catalog manager — reading the catalog + picking a
  // built-in (which writes `agent.config`) needs the def read caps beside the config caps.
  "mcp:agent.def.list:call",
  "mcp:agent.def.get:call",
  "mcp:agent.invoke:call",
  "bus:chan/general:pub",
  "bus:chan/general:sub",
];

beforeAll(() => useRealGateway());

describe("Agent default runtime — set in Settings, honored by an omitted-runtime run (real gateway)", () => {
  it("an admin picks the default runtime; a run with NO explicit runtime resolves to it and settles", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    const session = await signInWithCaps("user:ada", ws, ADMIN_CAPS);

    // 1) Set the workspace default via the real Settings → Agent UI: pick the seeded in-house
    //    built-in (runtime `default`) from the catalog. Picking writes a real `agent.config.set` with
    //    that definition's runtime + endpoint — the catalog is the library, `agent.config` the pick.
    render(<SettingsHarness ws={ws} caps={session.caps} />);
    await user.click(screen.getByLabelText("Agent"));
    // The seeded in-house built-in over `default` — filtered to node-runnable, so it lists here.
    const useBtn = await screen.findByLabelText("pick builtin.in-house-glm-4.6");
    await user.click(useBtn);
    // The picked entry becomes Active (the list highlights the resolved selection).
    await screen.findByLabelText("definition builtin.in-house-glm-4.6");

    // The stored record now carries the choice (a read against the live gateway, not component state).
    await waitFor(async () => {
      const cfg = await getAgentConfig();
      expect(cfg?.default_runtime).toBe("default");
    });
    cleanup();

    // 2) Post a channel agent request with the `runtime` field OMITTED (the whole point — the run must
    //    resolve the workspace default rather than being told which runtime to use). This is the exact
    //    wire shape the composer emits when the picker is left on the stored default; posting it
    //    directly proves the omitted-runtime path against the real host, independent of the picker UI.
    const runJob = "run-omitted-1";
    const agentItem: Item = {
      id: runJob,
      channel: "general",
      author: "user:ada",
      body: JSON.stringify({ kind: "agent", goal: "summarize the incident", job: runJob }),
      ts: 1,
    };
    // Guard the intent: the posted body carries NO `runtime` key.
    expect(agentItem.body).not.toContain("runtime");
    await post(ws, "general", agentItem);

    await waitFor(async () => {
      const hist = await history(ws, "general");
      expect(hist.some((i) => i.body.includes('"kind":"agent"'))).toBe(true);
    });

    // 3) Drive the queued run through the real host reactor path; the run resolves the workspace
    //    default (`default` = the in-house loop) and posts a durable answer back into the channel.
    await drainAgentRuns();

    await waitFor(
      async () => {
        const hist = await history(ws, "general");
        const answer = hist.find((i) => i.id === `a:${runJob}`);
        expect(answer, "the omitted-runtime run posted a durable answer").toBeDefined();
        // It ran the resolved DEFAULT runtime (the unconfigured node's real canned text), NOT an
        // opaque `agent_error` — the stored default was honored, not rejected.
        expect(answer!.body).toContain('"kind":"agent_result"');
        expect(answer!.body).toContain('"runtime":"default"');
      },
      { timeout: 10_000 },
    );
  });
});
