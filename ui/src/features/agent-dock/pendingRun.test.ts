// Unit tests for the pending-run derivation (agent-dock scope) — find the newest agent request and its
// terminal state from a session's items. Uses the REAL payload encoders (mirrors the host wire shape).

import { describe, expect, it } from "vitest";

import type { Item } from "@/lib/channel/channel.types";
import { encodeAgent } from "@/lib/channel/payload.types";
import { latestPendingRun } from "./pendingRun";

function agentReq(job: string, goal: string, ts: number): Item {
  return { id: `user:ada-${ts}`, channel: "dock.x", author: "user:ada", body: encodeAgent(goal, job), ts };
}
function result(job: string, ts: number): Item {
  return {
    id: `a:${job}`,
    channel: "dock.x",
    author: "system:agent-worker",
    body: JSON.stringify({ kind: "agent_result", goal: "g", runtime: "default", job, answer: "hi" }),
    ts,
  };
}
function errorItem(job: string, error: string, ts: number): Item {
  return {
    id: `a:${job}`,
    channel: "dock.x",
    author: "system:agent-worker",
    body: JSON.stringify({ kind: "agent_error", goal: "g", error }),
    ts,
  };
}

describe("latestPendingRun", () => {
  it("returns none for an empty / chat-only session", () => {
    const r = latestPendingRun([]);
    expect(r.job).toBeNull();
    expect(r.hasResult).toBe(false);
  });

  it("finds the newest agent request, pending (no answer yet)", () => {
    const items = [agentReq("run-1", "first?", 1), agentReq("run-2", "second?", 3)];
    const r = latestPendingRun(items);
    expect(r.job).toBe("run-2");
    expect(r.goal).toBe("second?");
    expect(r.hasResult).toBe(false);
    expect(r.hasError).toBe(false);
  });

  it("marks Done when the durable agent_result for the newest run landed", () => {
    const items = [agentReq("run-2", "q", 1), result("run-2", 2)];
    const r = latestPendingRun(items);
    expect(r.hasResult).toBe(true);
    expect(r.hasError).toBe(false);
  });

  it("marks Error with the message when an agent_error landed", () => {
    const items = [agentReq("run-2", "q", 1), errorItem("run-2", "agent not permitted", 2)];
    const r = latestPendingRun(items);
    expect(r.hasError).toBe(true);
    expect(r.errorText).toBe("agent not permitted");
  });
});
